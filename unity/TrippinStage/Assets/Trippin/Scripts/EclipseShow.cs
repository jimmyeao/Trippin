// unity_eclipse: a total solar eclipse. A black disc hangs in front of a soft
// glow, and 96 streamers of corona flare out from its rim.
//  - Shape: each streamer's length follows the eased spectrum (a different
//    slice of the mix round the rim), plus a swell with the bass, so the
//    corona is a live spectrum wreath.
//  - Motion: the corona turns slowly on the smooth energy clock and swings
//    direction with the phrase.
//  - Luminance: the halo and every streamer follow the music's loudness; the
//    drop blooms a diamond ring (eased).
//  - Drops: a build tightens the corona dim; the drop flares it.
// The black disc uses the stage's dark structure material, so it must stay
// opaque to hide the glow behind it (check on the M2).
using UnityEngine;

namespace TrippinStage
{
    public sealed class EclipseShow : KitShow
    {
        const int Streamers = 96;
        const float Disc = 5f;
        BeamPool _beams;
        GlowPool _glows;

        protected override void Build()
        {
            Env(160f, 0.05f);
            var sphere = GameObject.CreatePrimitive(PrimitiveType.Sphere);
            Object.Destroy(sphere.GetComponent<Collider>());
            sphere.name = "disc";
            sphere.transform.SetParent(transform, false);
            sphere.transform.localPosition = new Vector3(0f, 10f, 0f);
            sphere.transform.localScale = Vector3.one * (Disc * 2f);
            sphere.GetComponent<Renderer>().sharedMaterial = structMat;
            sphere.GetComponent<Renderer>().shadowCastingMode = UnityEngine.Rendering.ShadowCastingMode.Off;
            _beams = new BeamPool(transform, beamMat, Streamers, "corona", false, 0.07f, 0.004f, 14f, 0.3f, 0.6f, 0.3f);
            _glows = new GlowPool(transform, glowMat, 3, "halo", false);
        }

        protected override void Frame(ShowState s, float dt)
        {
            float gain = rx.Gain();
            Quaternion face = cam.transform.rotation;
            var centre = new Vector3(0f, 10f, 0f);
            float flare = 1f + 0.8f * rx.impact;
            _glows.Set(0, centre + new Vector3(0f, 0f, 3f), 30f * flare, 0.1f, gain * 0.9f, face);
            _glows.Set(1, centre + new Vector3(0f, 0f, 3.5f), 18f * flare, 0.0f, gain * 1.1f, face);
            _glows.Set(2, centre + new Vector3(0f, 0f, 4f), 11f * (1f + 1.2f * rx.impact), 0.55f, gain * (0.8f + 1.6f * rx.impact), face);
            float rot = rx.clk * 0.01f + 0.3f * Mathf.Sin(rx.phrase);
            float tight = 1f - 0.35f * rx.tension;
            for (int i = 0; i < Streamers; i++)
            {
                float a = i / (float)Streamers * Mathf.PI * 2f + rot;
                float bin = Mathf.Abs(Mathf.Sin(a * 0.5f + 0.2f));
                float lvl = rx.Spec(bin);
                float len = (2.5f + 11f * lvl + 3f * rx.bassFast) * tight * (i % 3 == 0 ? 1.4f : 1f);
                var dir = new Vector3(Mathf.Cos(a), Mathf.Sin(a), 0f);
                Vector3 p0 = centre + dir * (Disc + 0.15f);
                Vector3 p1 = p0 + dir * len;
                _beams.Set(i, p0, p1, Kit.Hue(0.05f + 0.1f * lvl + (i % 5) * 0.03f), gain * (0.3f + 1.2f * lvl));
            }
            rig.Orbit(cam, rx, 40f, 10f, 10f, dt, 0.3f, 0.15f);
        }
    }
}
