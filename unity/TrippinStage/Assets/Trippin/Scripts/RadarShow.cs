// unity_radar: a radar scope. A rotating sweep with a fading trail crosses a
// circular wire grid; 36 contacts glow where the sweep has just passed and
// fade as it moves on, as on a phosphor screen.
//  - Shape: each contact swells with the eased level of its own spectrum
//    bin when the sweep lights it, so the picture is a live spectrum scan.
//  - Motion: the sweep angle IS the smooth energy clock (it turns faster as
//    the track builds and surges on a drop); the camera orbits above.
//  - Luminance: contacts, trail and grid follow the music's loudness.
//  - Drops: a build dims the scope; the drop sweeps it bright (eased).
using UnityEngine;

namespace TrippinStage
{
    public sealed class RadarShow : KitShow
    {
        const int Trail = 19, Blips = 36;
        const float Radius = 22f;
        Material _m;
        BeamPool _sweep;
        GlowPool _blips;

        protected override void Build()
        {
            Env(140f, 0.04f, false);
            Mesh mesh = Kit.GridMesh(192, 72, "scope");
            _m = new Material(membraneMat);
            _m.SetFloat("_Radius", Radius);
            _m.SetFloat("_Amp", 0f);
            Kit.Part(transform, "scope", mesh, _m, Vector3.zero, Vector3.one);
            _sweep = new BeamPool(transform, beamMat, Trail, "sweep", false, 0.1f, 0.0f, 20f, 0.25f, 0.99f, 0.3f);
            _blips = new GlowPool(transform, glowMat, Blips, "contact", false);
        }

        protected override void Frame(ShowState s, float dt)
        {
            float gain = rx.Gain(0.55f, 1.4f);
            _m.SetFloat("_Intensity", 1.6f * gain);
            _m.SetFloat("_Hue", 0.4f + 0.08f * Mathf.Sin(rx.phrase));
            float phi = rx.clk * 0.12f;
            var centre = new Vector3(0f, 0.3f, 0f);
            for (int j = 0; j < Trail; j++)
            {
                float a = phi - j * 0.06f;
                float f = 1f - j / (float)Trail;
                _sweep.Set(j, centre, centre + new Vector3(Mathf.Cos(a), 0f, Mathf.Sin(a)) * Radius, Kit.Hue(0.4f), gain * 1.3f * f * f);
            }
            Quaternion face = cam.transform.rotation;
            for (int i = 0; i < Blips; i++)
            {
                float th = Kit.H(i, 1) * Mathf.PI * 2f;
                float rho = 4f + 16f * Kit.H(i, 2);
                float delta = (phi - th) % (Mathf.PI * 2f);
                if (delta < 0f) delta += Mathf.PI * 2f;
                float persist = Mathf.Exp(-delta * 1.4f);
                float lvl = rx.Spec(((i * 5) % 32) / 31f);
                float glow = 0.06f + persist * (0.4f + 1.4f * lvl);
                _blips.Set(i, new Vector3(Mathf.Cos(th) * rho, 0.5f, Mathf.Sin(th) * rho), 0.8f + 1.4f * lvl * persist, 0.2f + 0.5f * Kit.H(i, 3), gain * glow, face);
            }
            rig.Orbit(cam, rx, 30f, 24f, 0f, dt);
        }
    }
}
