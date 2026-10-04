// unity_robot_rave: seven chrome-and-ceramic androids dancing on a lit LED
// dance floor in front of a wall of LEDs, under overhead spotlights, every
// one with its own routine (arm pumps, snapping robot-dance, waves, claps,
// pogo) and its own band.
//  - Shape: each android's moves swell with the eased level of its band (bass,
//    mids, mid-highs, highs): its arms reach higher and its sway widens; its
//    visor is a live spectrum and its chest core and spotlight follow its band;
//    the floor lights up in rings from the bass and the spectrum round its edge.
//  - Motion: all of them dance on the smooth energy clock (so they speed up with
//    the track and surge on a drop), bounce on the eased kick, and reverse their
//    sway on a phrase-length sine; the spotlights sweep and the camera drifts.
//  - Luminance: robots, floor, wall and lights follow the music's loudness.
//  - Drops: a build tightens the line and dims it; the drop throws every android's
//    arms up and flares the floor (eased).
using UnityEngine;

namespace TrippinStage
{
    public sealed class RobotRaveShow : KitShow
    {
        const int N = 7;
        const float H = 7f;
        static readonly Quaternion Face = Quaternion.Euler(0f, 180f, 0f);
        static readonly Vector3[] Spots =
        {
            new Vector3(0f, 0f, 9f), new Vector3(-5.5f, 0f, 6.5f), new Vector3(5.5f, 0f, 6.5f),
            new Vector3(-11f, 0f, 3.5f), new Vector3(11f, 0f, 3.5f), new Vector3(-16.5f, 0f, 0.5f), new Vector3(16.5f, 0f, 0.5f),
        };
        RobotMats _mats;
        readonly Android[] _bot = new Android[N];
        Material _floorM, _wallM;
        BeamPool _spots, _wash;
        GlowPool _sparks;
        float _up;

        protected override void Build()
        {
            Env(150f, 0.05f);
            _mats = new RobotMats(robotMat);
            for (int i = 0; i < N; i++) _bot[i] = new Android(transform, _mats, H, Face, true, "bot" + i, i % 2 == 0);
            _floorM = Kit.Part(transform, "led floor", Kit.GridMesh(1, 1, "floor"), new Material(screenMat), new Vector3(0f, 0.04f, 6f), new Vector3(60f, 40f, 1f))
                .GetComponent<Renderer>().sharedMaterial;
            _floorM.SetFloat("_Mode", 1f);
            _floorM.SetFloat("_Cols", 90f);
            _floorM.SetFloat("_Rows", 60f);
            _floorM.SetFloat("_Rows", 60f);
            var floorT = transform.Find("led floor");
            floorT.localRotation = Quaternion.Euler(90f, 0f, 0f);
            _wallM = Kit.Part(transform, "led wall", Kit.GridMesh(1, 1, "wall"), new Material(screenMat), new Vector3(0f, 13f, 22f), new Vector3(72f, 26f, 1f))
                .GetComponent<Renderer>().sharedMaterial;
            _wallM.SetFloat("_Mode", 0f);
            _wallM.SetFloat("_Cols", 96f);
            _wallM.SetFloat("_Rows", 36f);
            _spots = new BeamPool(transform, beamMat, N, "spot", false, 0.14f, 0.012f, 8f, 0.65f, 0.55f, 0f);
            _wash = new BeamPool(transform, beamMat, 10, "wash", false, 0.05f, 0f, 40f, 0.45f, 0.98f, 0.3f);
            _sparks = new GlowPool(transform, glowMat, N, "floor glow", false);
        }

        // Smooth staircase: holds each integer, moves to the next quickly (the robot-dance snap).
        static float Stair(float x)
        {
            float f = Mathf.Floor(x);
            return f + Mathf.SmoothStep(0f, 1f, Mathf.Clamp01((x - f - 0.62f) / 0.38f));
        }

        protected override void Frame(ShowState s, float dt)
        {
            float gain = rx.Gain(0.6f, 1.45f);
            float hue = 0.5f + 0.1f * Mathf.Sin(rx.phrase * 0.5f);
            _mats.Tune(hue, gain);
            float kick = rx.kick;
            float dir = Mathf.Clamp(Mathf.Sin(rx.phrase) * 4f, -1f, 1f);       // reverses with the phrase, smoothly
            _up = Mathf.Clamp01(rx.impact * 2.2f);
            float tight = 1f - 0.18f * rx.tension;
            Quaternion face = cam.transform.rotation;

            for (int i = 0; i < N; i++)
            {
                int band = i & 3;
                float amp = 0.45f + 0.7f * rx.Band(band);
                float ph = i * 0.9f;
                float t = rx.clk;
                Vector3 baseP = new Vector3(Spots[i].x * tight, 0f, Spots[i].z);
                float bob = Mathf.Abs(Mathf.Sin(t * Mathf.PI * 0.5f + ph));
                float jump = (i % 5 == 4) ? Mathf.Max(0f, Mathf.Sin(t * Mathf.PI + ph)) * 0.14f * H * amp : 0f;
                Vector3 pel = baseP + new Vector3(0.4f * Mathf.Sin(t * 0.5f + ph) * dir, 0.51f * H - 0.045f * H * kick * amp - 0.035f * H * bob + jump, 0f);

                // Local (robot frame, in units of H, +z forward, +x the android's right) hand targets.
                Vector3 lh = new Vector3(-0.16f, -0.03f, 0.03f), rh = new Vector3(0.16f, -0.03f, 0.03f);
                switch (i % 5)
                {
                    case 0:     // alternating pumps overhead
                    {
                        float a = Mathf.Max(0f, Mathf.Sin(t * Mathf.PI * 0.5f + ph)), b = Mathf.Max(0f, Mathf.Sin(t * Mathf.PI * 0.5f + ph + Mathf.PI));
                        lh = new Vector3(-0.2f, 0.18f + 0.32f * a * amp, 0.1f); rh = new Vector3(0.2f, 0.18f + 0.32f * b * amp, 0.1f);
                        break;
                    }
                    case 1:     // robot dance: arms snap through right angles
                    {
                        float a = Stair(t * 0.35f + i) * Mathf.PI * 0.5f;
                        lh = new Vector3(-0.2f - 0.1f * Mathf.Cos(a) * amp, 0.28f + 0.2f * Mathf.Sin(a) * amp, 0.06f);
                        float b = Stair(t * 0.35f + i + 0.5f) * Mathf.PI * 0.5f;
                        rh = new Vector3(0.2f + 0.1f * Mathf.Cos(b) * amp, 0.28f + 0.2f * Mathf.Sin(b) * amp, 0.06f);
                        break;
                    }
                    case 2:     // arms out, waving
                    {
                        float w1 = Mathf.Sin(t * Mathf.PI * 0.5f + ph), w2 = Mathf.Sin(t * Mathf.PI * 0.5f + ph + 0.8f);
                        lh = new Vector3(-0.3f, 0.3f + 0.12f * w1 * amp, 0.05f); rh = new Vector3(0.3f, 0.3f + 0.12f * w2 * amp, 0.05f);
                        break;
                    }
                    case 3:     // hands over the head, clapping on the kick
                    {
                        float open = 0.04f + 0.14f * (1f - kick) * (0.4f + 0.6f * amp);
                        lh = new Vector3(-open, 0.5f + 0.04f * amp, 0.1f); rh = new Vector3(open, 0.5f + 0.04f * amp, 0.1f);
                        break;
                    }
                    default:    // pogo fist pumps
                    {
                        float a = Mathf.Max(0f, Mathf.Sin(t * Mathf.PI + ph));
                        lh = new Vector3(-0.2f, 0.22f + 0.28f * a * amp, 0.12f); rh = new Vector3(0.2f, 0.22f + 0.28f * a * amp, 0.12f);
                        break;
                    }
                }
                // A drop sends everyone's arms up.
                Vector3 upL = new Vector3(-0.22f, 0.52f + 0.04f * Mathf.Sin(t * 0.9f + ph), 0.08f), upR = new Vector3(0.22f, 0.52f + 0.04f * Mathf.Sin(t * 0.9f + ph + 1f), 0.08f);
                lh = Vector3.Lerp(lh, upL, _up); rh = Vector3.Lerp(rh, upR, _up);

                float lift = (i % 5 == 4) ? Mathf.Max(0f, Mathf.Sin(t * Mathf.PI + ph)) * 0.1f * H * amp : 0f;
                float foot = 0.04f * H;
                var pose = new AndroidPose
                {
                    pelvis = pel,
                    yaw = 12f * Mathf.Sin(t * 0.25f + ph) * dir,
                    chest = new Vector3(5f + 8f * kick * amp, 22f * Mathf.Sin(t * Mathf.PI * 0.25f + ph) * dir * amp, 6f * Mathf.Sin(t * Mathf.PI * 0.5f + ph)),
                    head = new Vector3(6f * kick + 3f * Mathf.Sin(t * 0.6f + ph), 18f * Mathf.Sin(t * 0.3f + ph + 1f), 4f * Mathf.Sin(t * 0.4f + ph)),
                    handL = baseP + new Vector3(0f, pel.y, 0f) + Face * (lh * H),
                    handR = baseP + new Vector3(0f, pel.y, 0f) + Face * (rh * H),
                    footL = baseP + Face * new Vector3(-0.07f * H, 0f, 0.02f * H) + new Vector3(0f, foot + lift, 0f),
                    footR = baseP + Face * new Vector3(0.07f * H, 0f, 0.02f * H) + new Vector3(0f, foot, 0f),
                    eye = 0.4f + 0.6f * rx.Band(band),
                    core = 0.3f + 0.8f * rx.Band(band) + 0.4f * rx.impact,
                };
                _bot[i].Apply(pose, hue + 0.09f * i, rx);

                // Spotlight from the truss down onto this android, a glow on the floor under it.
                var from = new Vector3(baseP.x + 3f * Mathf.Sin(rx.phrase + i), 26f, baseP.z + 3f);
                _spots.Set(i, from, new Vector3(baseP.x, 0f, baseP.z), Kit.Hue(hue + 0.09f * i), gain * (0.07f + 0.22f * rx.Band(band) + 0.2f * rx.impact));
                _sparks.Set(i, baseP + new Vector3(0f, 0.1f, 0f), 6f + 4f * rx.Band(band), hue + 0.09f * i, gain * (0.2f + 0.6f * rx.Band(band)), Quaternion.Euler(90f, 0f, 0f));
            }

            _floorM.SetFloat("_Hue", hue);
            _floorM.SetFloat("_Gain", gain);
            _floorM.SetFloat("_Clk", rx.clk);
            _wallM.SetFloat("_Hue", hue + 0.1f);
            _wallM.SetFloat("_Gain", gain * 0.9f);
            _wallM.SetFloat("_Clk", rx.clk);
            for (int i = 0; i < 10; i++)
            {
                float u = (i - 4.5f) / 4.5f;
                var from = new Vector3(u * 28f, 0.4f, 19f);
                float ang = u * 0.8f + 0.5f * Mathf.Sin(rx.phrase + i * 0.5f);
                _wash.Set(i, from, from + new Vector3(Mathf.Sin(ang) * 20f, 24f, -6f * Mathf.Cos(ang)), Kit.Hue(hue + 0.06f * i), gain * (0.25f + 0.6f * rx.midFast));
            }

            float sway = Mathf.Sin(rx.phrase * 0.5f);
            rig.Move(cam, new Vector3(sway * 9f, 6.5f + 1.2f * Mathf.Sin(rx.phrase), -27f + 5f * rx.tension),
                new Vector3(0f, 4.6f, 5f), dt, 0.6f);
        }
    }
}
