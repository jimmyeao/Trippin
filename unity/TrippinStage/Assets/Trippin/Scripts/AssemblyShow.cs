// unity_assembly: eight industrial robot arms ringed round a plasma core on a
// circular LED floor, tending it like a factory line: their tool tips trace
// paths round the core, throw sparks on the kick, and drag the core's light
// out of it. Overhead lights converge on the core.
//  - Shape: the reach of every arm (how far its tip stands off the core) and the
//    core's surface both follow the eased bass; each arm's seams, its tool tip
//    and its overhead light follow its own band; the floor rings spread from the
//    bass and the spectrum lights its edge.
//  - Motion: the tips orbit and swoop on the smooth energy clock (faster as the
//    track builds, surging on a drop) and reverse direction with the phrase;
//    sparks fly off the tips on the eased kick; the camera orbits.
//  - Luminance: the arms, core, floor and lights follow the music's loudness.
//  - Drops: a build pulls the tips in tight and dim; the drop throws them out and
//    flares the core (eased).
using UnityEngine;

namespace TrippinStage
{
    public sealed class AssemblyShow : KitShow
    {
        const int Arms = 8, Sparks = 5;
        const float Ring = 15f;
        static readonly Vector3 Core = new Vector3(0f, 9f, 0f);
        RobotMats _mats;
        PartPool _cer, _chr, _dark, _tip, _seam;
        GlowPool _sparks;
        BeamPool _lights;
        Material _core, _coreM, _floorM;

        protected override void Build()
        {
            Env(150f, 0.06f, true);
            _mats = new RobotMats(robotMat);
            _cer = new PartPool(transform, _mats.ceramic, PartPool.Kind.Cylinder, Arms * 3, "arm link", true);
            _chr = new PartPool(transform, _mats.chrome, PartPool.Kind.Sphere, Arms * 3, "arm joint", true);
            _dark = new PartPool(transform, _mats.dark, PartPool.Kind.Cylinder, Arms * 2, "arm base", true);
            _tip = new PartPool(transform, _mats.glow, PartPool.Kind.Sphere, Arms, "tool tip", true);
            _seam = new PartPool(transform, _mats.glow, PartPool.Kind.Cylinder, Arms * 2, "arm seam", true);
            _sparks = new GlowPool(transform, glowMat, Arms * Sparks, "spark", false);
            _lights = new BeamPool(transform, beamMat, 12, "overhead", false, 0.12f, 0.01f, 10f, 0.6f, 0.5f, 0f);
            _floorM = Kit.Part(transform, "led floor", Kit.GridMesh(1, 1, "floor"), new Material(screenMat), new Vector3(0f, 0.04f, 0f), new Vector3(48f, 48f, 1f))
                .GetComponent<Renderer>().sharedMaterial;
            transform.Find("led floor").localRotation = Quaternion.Euler(90f, 0f, 0f);
            _floorM.SetFloat("_Mode", 1f);
            _floorM.SetFloat("_Cols", 80f);
            _floorM.SetFloat("_Rows", 80f);
            Mesh mesh = SculptureShow.IcoSphere(5);
            mesh.bounds = new Bounds(Vector3.zero, Vector3.one * 40f);
            _core = new Material(orbMat); _coreM = new Material(orbMat);
            _core.SetFloat("_Radius", 3.6f); _coreM.SetFloat("_Radius", 3.6f);
            Kit.Part(transform, "core", mesh, _core, Core, Vector3.one);
            Kit.Part(transform, "core reflection", mesh, _coreM, new Vector3(Core.x, -Core.y, Core.z), new Vector3(1f, -1f, 1f));
        }

        protected override void Frame(ShowState s, float dt)
        {
            float gain = rx.Gain(0.6f, 1.45f);
            float hue = 0.08f + 0.06f * Mathf.Sin(rx.phrase * 0.5f);      // warm industrial palette
            _mats.Tune(hue, gain);
            float kick = rx.kick, bass = rx.bassFast;
            float dir = Mathf.Clamp(Mathf.Sin(rx.phrase) * 4f, -1f, 1f);
            float reach = (5.4f + 2.2f * bass + 1.2f * rx.impact) * (1f - 0.25f * rx.tension);

            _core.SetFloat("_Amp", 0.7f + 1.4f * bass + 0.7f * rx.impact);
            _core.SetFloat("_Freq", 1.3f + 1.1f * rx.highFast);
            _core.SetFloat("_Phase", rx.clk * 0.3f);
            _core.SetFloat("_Hue", hue + 0.1f);
            _core.SetFloat("_Intensity", 0.8f * gain);
            _coreM.CopyPropertiesFromMaterial(_core);
            _coreM.SetFloat("_Intensity", 0.8f * gain * 0.3f);

            Quaternion face = cam.transform.rotation;
            for (int j = 0; j < Arms; j++)
            {
                float a = j / (float)Arms * Mathf.PI * 2f + 0.3f;
                int band = j & 3;
                float lvl = rx.Band(band);
                Vector3 B = new Vector3(Mathf.Cos(a) * Ring, 0f, Mathf.Sin(a) * Ring);
                Vector3 S = B + new Vector3(0f, 4.2f, 0f);
                // The tool tip orbits and swoops round the core.
                float th = a + Mathf.PI + 0.9f * dir * Mathf.Sin(rx.clk * 0.07f + j) + 0.5f * Mathf.Sin(rx.clk * 0.21f + j * 1.7f);
                float el = 0.5f * Mathf.Sin(rx.clk * 0.13f + j * 2.1f) + 0.2f;
                float r = reach * (0.8f + 0.5f * lvl);
                Vector3 T = Core + new Vector3(Mathf.Cos(a) * Mathf.Cos(el) * r * (1f - 0.3f * Mathf.Cos(th - a - Mathf.PI)),
                                                Mathf.Sin(el) * r, Mathf.Sin(a) * Mathf.Cos(el) * r);
                // Swing round the core a little way.
                float swing = 0.7f * Mathf.Sin(rx.clk * 0.09f * dir + j);
                float ca = Mathf.Cos(swing), sa = Mathf.Sin(swing);
                Vector3 rel = T - Core;
                T = Core + new Vector3(rel.x * ca - rel.z * sa, rel.y, rel.x * sa + rel.z * ca);

                Vector3 wrist;
                Vector3 elbow = Android.Ik(S, T, 8.8f, 8.2f, new Vector3(Mathf.Cos(a) * 0.8f, 1.2f, Mathf.Sin(a) * 0.8f), out wrist);
                _dark.Cyl(j * 2, B, B + new Vector3(0f, 1.0f, 0f), 2.6f);
                _cer.Cyl(j * 3, B + new Vector3(0f, 1.0f, 0f), S, 1.7f);
                _cer.Cyl(j * 3 + 1, S, elbow, 1.15f);
                _cer.Cyl(j * 3 + 2, elbow, wrist, 0.9f);
                _chr.Ball(j * 3, S, 1.9f);
                _chr.Ball(j * 3 + 1, elbow, 1.45f);
                _chr.Ball(j * 3 + 2, wrist, 1.0f);
                Vector3 toolDir = (T - wrist).sqrMagnitude > 1e-4f ? (T - wrist).normalized : Vector3.down;
                Vector3 tipP = wrist + toolDir * 3.0f;
                _dark.Cyl(j * 2 + 1, wrist, tipP, 0.38f);
                _tip.Ball(j, tipP, 0.55f + 0.4f * kick);
                _tip.Glow(j, hue + 0.05f + 0.1f * j, 1.5f + 3f * lvl + 2f * kick);
                _seam.Cyl(j * 2, S + (elbow - S) * 0.45f, S + (elbow - S) * 0.55f, 1.22f);
                _seam.Glow(j * 2, hue + 0.1f * j, 0.4f + 1.8f * lvl);
                _seam.Cyl(j * 2 + 1, elbow + (wrist - elbow) * 0.45f, elbow + (wrist - elbow) * 0.55f, 0.97f);
                _seam.Glow(j * 2 + 1, hue + 0.1f * j, 0.4f + 1.8f * lvl);
                for (int k = 0; k < Sparks; k++)
                {
                    float u = Kit.H(j * 7 + k, 3);
                    Vector3 d = new Vector3(Kit.H(j * 13 + k, 4) - 0.5f, Kit.H(j * 13 + k, 5) * 0.8f - 0.1f, Kit.H(j * 13 + k, 6) - 0.5f).normalized;
                    float life = Mathf.Repeat(rx.clk * 0.6f + u, 1f);
                    float spread = kick * (0.4f + 3.2f * life);
                    _sparks.Set(j * Sparks + k, tipP + d * spread + new Vector3(0f, -2f * life * life * kick, 0f), 0.35f + 0.3f * u, hue + 0.05f, gain * 1.2f * kick * (1f - life), face);
                }
            }
            for (int i = 0; i < 12; i++)
            {
                float a = i / 12f * Mathf.PI * 2f;
                var from = new Vector3(Mathf.Cos(a) * 24f, 24f, Mathf.Sin(a) * 24f);
                _lights.Set(i, from, Core + new Vector3(Mathf.Sin(rx.phrase + i) * 1.2f, 0f, 0f), Kit.Hue(hue + 0.04f * (i % 6)), gain * (0.08f + 0.3f * rx.Spec(i / 11f) + 0.25f * rx.impact));
            }
            _floorM.SetFloat("_Hue", hue + 0.05f);
            _floorM.SetFloat("_Gain", gain);
            _floorM.SetFloat("_Clk", rx.clk);
            rig.Orbit(cam, rx, 56f, 22f, 9f, dt, 1f, 0.15f);
        }
    }
}
