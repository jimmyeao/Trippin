// unity_meteors: the HQ version of the 2D comets scene: a meteor shower
// over a still alpine lake at night (an Alice backdrop with the Milky Way),
// meteors burning across the sky (GPU sparks with long trails) and mirrored
// in the water, which ripples their reflections.
//  - Shape: the bass draws the trails out longer; each kick sends a bright
//    meteor from the shower's radiant; the drop a fireball that breaks up
//    in a spray of embers; breakdowns thin to the odd faint streak.
//  - Motion: the radiant drifts across the sky, swinging with the phrase,
//    so the shower's direction turns; the camera drifts slightly.
// Controls: meteors, trail length, brightness, colour, fireballs, ripple.
using UnityEngine;

namespace TrippinStage
{
    public sealed class MeteorsShow : KitShow
    {
        public Material sparkMat, backMat;
        public ComputeShader particles;

        [TrippinParam("Meteors", 0f, 3f)] public float rate = 1f;
        [TrippinParam("Trail length", 0.3f, 2.5f)] public float trail = 1f;
        [TrippinParam("Brightness", 0.3f, 2f)] public float bright = 1f;
        [TrippinParam("Colour", 0f, 1f)] public float colour = 0.5f;
        [TrippinParam("Fireballs", 0f, 2f)] public float fireballs = 1f;
        [TrippinParam("Ripple", 0f, 2.5f)] public float ripple = 1f;

        public override bool UsesCamera => true;

        Material _back;
        ParticleField _met;
        float _kickPrev, _acc, _len;

        protected override void Build()
        {
            Env(120f, 0f, false);
            _back = new Material(backMat);
            _back.SetFloat("_URange", 1f);
            _back.SetFloat("_EdgeFade", 0.05f);
            _back.SetFloat("_Haze", 0f);
            // The image's far shoreline (44% up it) sits at eye level, the horizon
            // the meteors' reflections are mirrored about.
            HqBackdrops.Make(transform, _back, null, new Vector3(0f, -37.1f, 75f), 154f, 154f * 768f / 1344f);
            _met = new ParticleField(particles, sparkMat, 65536, true, 0f);
            _met.Gravity = Vector3.zero;
        }

        void OnDestroy() { _met?.Release(); }

        Color Tint(float k)
        {
            // Meteors burn green, blue-white or orange, by their make-up.
            var a = new Color(0.55f, 1.6f, 0.9f);
            var b = new Color(1.3f, 1.35f, 1.6f);
            var c = new Color(1.7f, 0.9f, 0.4f);
            var nat = k < 0.33f ? a : k < 0.66f ? b : c;
            return Color.Lerp(b, nat, colour) * bright;
        }

        void Meteor(float strength, bool fireball)
        {
            // From near the radiant, a random way out of it. The shower stays
            // high and nearer than the backdrop, so it never crosses the peaks.
            float swing = Mathf.Sin(rx.phrase * 0.5f);
            var radiant = new Vector3(-18f + 14f * swing, 34f, 62f);
            var start = radiant + new Vector3(Random.Range(-38f, 38f), Random.Range(-8f, 6f), Random.Range(-8f, 6f));
            var dir = (start - radiant).normalized + new Vector3(0f, -0.25f, -0.1f);
            dir.Normalize();
            float speed = Random.Range(18f, 30f) * (fireball ? 0.7f : 1f);
            float life = Random.Range(0.5f, 1.1f) * (fireball ? 1.8f : 1f);
            var col = Tint(Random.value) * (1.5f + 2.5f * strength) * (fireball ? 2f : 1f);
            float size = (fireball ? 0.45f : 0.2f) * (0.6f + 0.6f * strength);
            _met.Burst(start, 1, ParticleField.Shape.Cone, speed, size, life, col, 0f, 0f, default, -dir, 0f,
                       24, 0.3f * trail * (0.6f + 0.8f * _len) * (fireball ? 1.6f : 1f), 0.001f);
            if (fireball)
            {
                // It breaks up near the end of its run: embers flung forward.
                var at = start + dir * speed * life * 0.75f;
                _met.Burst(at, 70, ParticleField.Shape.Cone, speed * 0.35f, 0.35f, 1.2f, col * 0.6f, 0.6f, 0f, default, -dir, 0.6f, 4, 0.12f, 0.35f);
            }
        }

        protected override void Frame(ShowState s, float dt)
        {
            dt = Mathf.Clamp(dt, 1e-4f, 1f / 20f);
            bool calm = rx.calm > 0.5f;
            _len = Eased.Follow(_len, rx.bassFast, 10f, 2f, dt);
            bool kick = rx.kick > 0.55f && _kickPrev <= 0.55f && !calm;
            _kickPrev = rx.kick;
            if (kick && rate > 0f) { Meteor(1f, false); Meteor(0.7f, false); }
            if (rx.dropped && fireballs > 0f) { Meteor(1f, true); if (fireballs > 1f) Meteor(0.8f, true); }
            // A background trickle of faint streaks, more as the track drives.
            _acc += dt * rate * (calm ? 0.8f : 2f + 4f * rx.lum);
            while (_acc >= 1f) { _acc -= 1f; Meteor(Random.Range(0.1f, 0.5f), false); }

            float gain = rx.Gain(0.85f, 1.2f);
            _met.MirrorMat.SetFloat("_Shimmer", ripple * 2.2f);
            _met.Tick(dt, gain, 0.035f, 0f);
            _back.SetFloat("_Gain", 1.1f * (0.92f + 0.12f * rx.lum));
            _back.SetFloat("_Lit", 0.9f);

            // Level, so the horizon sits on the lake's far shore.
            var camPos = new Vector3(Mathf.Sin(rx.phrase * 0.25f) * 0.6f, 1.6f, -2f);
            var look = camPos + new Vector3(0f, 0f, 20f);
            rig.Move(cam, camPos, look, dt, 0.4f);
        }
    }
}
