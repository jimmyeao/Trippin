// unity_robot_band: a four-piece band of androids on a lit stage: a drummer
// behind a full kit, a bassist, a keyboard player and a singer at the front,
// under a laser fan, in front of an LED wall, reflected in the floor.
//  - Shape: every drum head flashes (emission) on its own hit and the kit's
//    cymbals swing; the keys that are pressed light up; the bass strings and
//    the singer's mic head follow the eased mids; each android's visor is a live
//    spectrum and its chest core swells with its own band; the LED wall is the
//    spectrum.
//  - Motion: the whole band plays on the BEAT (Rx.beatS, the tracked beat made
//    continuous), not the energy clock: the kick pedal on every beat, the snare
//    on 2 and 4, the hi-hat on the half beats, a crash on the bar line every
//    four bars, the bassist's fretting hand changing notes every two beats and
//    plucking on the half beats, the keyboard player's arpeggio stepping a key
//    every half beat, the singer pumping an arm every two beats; everybody nods
//    on the beat. They all stop playing in a breakdown (the calm), arms drop,
//    and come back in on the drop.
//  - Luminance: the rig, the heads, the wall follow the music's loudness.
//  - Drops: a build tightens and dims the rig; the drop throws the singer's arms
//    up and flares the wall (eased).
using UnityEngine;

namespace TrippinStage
{
    public sealed class RobotBandShow : KitShow
    {
        const float H = 8f;
        static readonly Quaternion Face = Quaternion.Euler(0f, 180f, 0f);
        RobotMats _mats;
        Android _drum, _bass, _keys, _vox;
        // Props.
        PartPool _shell, _skin, _cym, _dark, _keyTop, _key, _neon, _stand;
        Material _wallM, _floorM;
        BeamPool _lasers;
        const int Keys = 16;

        // Drummer geometry (world).
        static readonly Vector3 DrumPelvis = new Vector3(0f, 3.07f, 14f);
        static readonly Vector3 Snare = new Vector3(0.7f, 3.9f, 11.8f), HiHat = new Vector3(-1.9f, 4.4f, 12.3f);
        static readonly Vector3 TomA = new Vector3(-0.5f, 4.5f, 11.4f), TomB = new Vector3(1.9f, 4.4f, 11.9f), Crash = new Vector3(-3.0f, 6.0f, 12.4f);

        protected override void Build()
        {
            Env(160f, 0.06f);
            _mats = new RobotMats(robotMat);
            _drum = new Android(transform, _mats, H, Face, true, "drummer", true);
            _bass = new Android(transform, _mats, H, Face, true, "bassist");
            _keys = new Android(transform, _mats, H, Face, true, "keys");
            _vox = new Android(transform, _mats, H, Face, true, "singer", true);
            _shell = new PartPool(transform, _mats.chrome, PartPool.Kind.Cylinder, 7, "drum shell", true);
            _skin = new PartPool(transform, _mats.glow, PartPool.Kind.Cylinder, 7, "drum head", true);
            _cym = new PartPool(transform, _mats.chrome, PartPool.Kind.Cylinder, 4, "cymbal", true);
            _dark = new PartPool(transform, _mats.dark, PartPool.Kind.Cube, 8, "prop", true);
            _key = new PartPool(transform, _mats.ceramic, PartPool.Kind.Cube, Keys, "key", true);
            _keyTop = new PartPool(transform, _mats.glow, PartPool.Kind.Cube, Keys, "key light", true);
            _neon = new PartPool(transform, _mats.glow, PartPool.Kind.Cube, 6, "neon", true);
            _stand = new PartPool(transform, _mats.chrome, PartPool.Kind.Cylinder, 8, "stand", true);
            _wallM = Kit.Part(transform, "led wall", Kit.GridMesh(1, 1, "wall"), new Material(screenMat), new Vector3(0f, 12f, 22f), new Vector3(64f, 24f, 1f))
                .GetComponent<Renderer>().sharedMaterial;
            _wallM.SetFloat("_Mode", 0f);
            _wallM.SetFloat("_Cols", 96f);
            _wallM.SetFloat("_Rows", 36f);
            _floorM = Kit.Part(transform, "led floor", Kit.GridMesh(1, 1, "floor"), new Material(screenMat), new Vector3(0f, 0.04f, 9f), new Vector3(40f, 24f, 1f))
                .GetComponent<Renderer>().sharedMaterial;
            transform.Find("led floor").localRotation = Quaternion.Euler(90f, 0f, 0f);
            _floorM.SetFloat("_Mode", 1f);
            _floorM.SetFloat("_Cols", 80f);
            _floorM.SetFloat("_Rows", 48f);
            _lasers = new BeamPool(transform, beamMat, 12, "laser", false, 0.05f, 0f, 40f, 0.45f, 0.98f, 0.3f);
        }

        // 0 on the beat (a strike lands), 1 half a period later: a hand or foot height factor.
        static float Strike(float beat, float period, float offset = 0f) =>
            0.5f - 0.5f * Mathf.Cos(2f * Mathf.PI * (beat - offset) / period);

        // Envelope that is 1 at the instant of the strike and decays: heads and cymbals flash on it.
        static float Hit(float beat, float period, float offset = 0f, float rate = 9f)
        {
            float p = Mathf.Repeat((beat - offset) / period, 1f) * period;      // beats since the strike
            return Mathf.Exp(-p * rate);
        }

        Vector3 W(Vector3 origin, Vector3 local) => origin + Face * local;

        protected override void Frame(ShowState s, float dt)
        {
            float gain = rx.Gain(0.6f, 1.45f);
            float hue = 0.5f + 0.08f * Mathf.Sin(rx.phrase * 0.5f);
            _mats.Tune(hue, gain);
            float b = rx.beatS;
            float play = 1f - Mathf.Clamp01(rx.calm * 1.6f);          // the band lays out in a breakdown
            float tight = 1f - 0.12f * rx.tension;
            float kick = rx.kick;

            DrawDrums(b, play, hue, gain, kick);
            DrawBass(b, play, hue, gain);
            DrawKeys(b, play, hue, gain);
            DrawVox(b, play, hue, gain, kick);

            // The wall, the floor and the lasers.
            _wallM.SetFloat("_Hue", hue + 0.05f); _wallM.SetFloat("_Gain", gain * 0.9f); _wallM.SetFloat("_Clk", rx.clk);
            _floorM.SetFloat("_Hue", hue); _floorM.SetFloat("_Gain", gain); _floorM.SetFloat("_Clk", rx.clk);
            int form = ((int)(b / 16f)) % 3;
            for (int i = 0; i < 12; i++)
            {
                float u = (i - 5.5f) / 5.5f;
                var from = new Vector3(u * 20f * tight, 0.4f, 20f);
                float ang = form == 0 ? u * 0.8f + 0.5f * Mathf.Sin(rx.phrase) : (form == 1 ? -u * 0.6f + 0.4f * Mathf.Sin(rx.phrase + i * 0.4f) : 0.45f * Mathf.Sin(rx.phrase * 2f + i * 0.9f));
                _lasers.Set(i, from, from + new Vector3(Mathf.Sin(ang) * 22f, 26f, -6f * Mathf.Cos(ang)), Kit.Hue(hue + 0.06f * (i % 5)), gain * (0.25f + 0.6f * rx.midFast + 0.4f * rx.impact) * (0.4f + 0.6f * play));
            }

            float sway = Mathf.Sin(rx.phrase * 0.5f);
            rig.Move(cam, new Vector3(sway * 6f, 5.4f + 0.8f * Mathf.Sin(rx.phrase), -13f + 3f * rx.tension), new Vector3(0f, 5.0f, 9f), dt, 0.6f);
        }

        void DrawDrums(float b, float play, float hue, float gain, float kick)
        {
            var P = DrumPelvis + new Vector3(0f, -0.07f * H * Strike(b, 1f) * play, 0f);
            float amp = play * (0.6f + 0.4f * rx.Band(0));
            // Hands: right (world -x) rides the hi-hat on the half beats; left (world +x) the snare on 2 and 4.
            float hh = Strike(b, 0.5f);
            Vector3 hRight = HiHat + new Vector3(0f, 0.3f + 0.55f * hh * amp, 0f);
            float sn = Strike(b, 2f, 1f);
            Vector3 hLeft = Snare + new Vector3(0f, 0.3f + 1.2f * sn * amp, 0f);
            // A crash on the bar line, every four bars: the left hand goes up to the cymbal and hits it.
            float r = Mathf.Repeat(b, 16f);
            float d = Mathf.Min(r, 16f - r);
            float wc = (1f - Mathf.Clamp01(d / 0.8f)) * play;
            Vector3 crashHit = Crash + new Vector3(0.5f, 0.2f + 0.7f * Mathf.Clamp01(d / 0.4f), -0.5f);
            hLeft = Vector3.Lerp(hLeft, crashHit, wc);
            // Feet, on the pedals behind the kick drum: the left foot (world +x: the android faces -z)
            // on the kick pedal every beat, the right foot on the hi-hat pedal (the hi-hat is on this
            // kit's right) on 2 and 4. Planted just under the knees, so the shins stay clear of the shell.
            float kp = Strike(b, 1f);
            var footL = new Vector3(0.35f, 0.3f + 0.45f * kp * amp, P.z - 1.85f);
            var footR = new Vector3(-1.5f, 0.3f + 0.3f * Strike(b, 2f, 1f), P.z - 1.7f);
            var pose = new AndroidPose
            {
                pelvis = P, yaw = 0f,
                chest = new Vector3(14f + 4f * kick, 6f * Mathf.Sin(b * Mathf.PI * 0.5f) * play, 3f * Mathf.Sin(b * Mathf.PI * 0.25f)),
                head = new Vector3(5f * Mathf.Cos(b * Mathf.PI * 2f) * play + 2f * kick, 10f * Mathf.Sin(b * Mathf.PI * 0.25f), 3f * Mathf.Sin(b * Mathf.PI * 0.5f)),
                handL = hLeft, handR = hRight, footL = footL, footR = footR,
                eye = 0.4f + 0.6f * rx.Band(0), core = 0.3f + 0.9f * rx.Band(0) + 0.4f * rx.impact,
            };
            _drum.Apply(pose, hue, rx);

            // The kit: kick, snare, two toms, hi-hat, crash. Heads flash on their own hit.
            float kickHit = Hit(b, 1f) * play, snareHit = Hit(b, 2f, 1f) * play, hhHit = Hit(b, 0.5f, 0f, 14f) * play;
            float tomHit = Hit(b, 4f, 3.5f) * play * (0.4f + 0.6f * rx.Band(1));
            float crashHitE = Hit(b, 16f, 0f, 3f) * play;
            // Kick drum (horizontal, facing the camera).
            // It stands in front of the drummer's knees (back head at z 11.55, knees at ~12.1), low
            // enough to clear the snare above it.
            _shell.Cyl(0, new Vector3(0f, 1.85f, 10.45f), new Vector3(0f, 1.85f, 11.55f), 1.5f);
            _skin.Cyl(0, new Vector3(0f, 1.85f, 10.43f), new Vector3(0f, 1.85f, 10.47f), 1.37f); _skin.Glow(0, hue, 0.3f + 3.0f * kickHit);
            _shell.Cyl(1, new Vector3(Snare.x, Snare.y - 0.35f, Snare.z), new Vector3(Snare.x, Snare.y, Snare.z), 1.0f);
            _skin.Cyl(1, new Vector3(Snare.x, Snare.y, Snare.z), new Vector3(Snare.x, Snare.y + 0.04f, Snare.z), 0.92f); _skin.Glow(1, hue + 0.12f, 0.3f + 3.2f * snareHit);
            _shell.Cyl(2, TomA + new Vector3(0f, -0.5f, 0f), TomA, 0.8f);
            _skin.Cyl(2, TomA, TomA + new Vector3(0f, 0.04f, 0f), 0.74f); _skin.Glow(2, hue + 0.25f, 0.3f + 2.4f * tomHit);
            _shell.Cyl(3, TomB + new Vector3(0f, -0.6f, 0f), TomB, 0.9f);
            _skin.Cyl(3, TomB, TomB + new Vector3(0f, 0.04f, 0f), 0.84f); _skin.Glow(3, hue + 0.35f, 0.3f + 2.4f * tomHit * 0.8f);
            // Hi-hat and crash cymbals (thin discs), the crash swinging when hit.
            _cym.Cyl(0, HiHat + new Vector3(0f, -0.06f - 0.05f * hh, 0f), HiHat + new Vector3(0f, 0.0f, 0f), 0.95f);
            _cym.Cyl(1, HiHat + new Vector3(0f, 0.18f + 0.1f * hh, 0f), HiHat + new Vector3(0f, 0.24f + 0.1f * hh, 0f), 0.95f);
            Vector3 tilt = new Vector3(0.5f * crashHitE, 0f, 0f);
            _cym.Cyl(2, Crash - tilt, Crash + tilt + new Vector3(0f, 0.08f, 0f), 1.5f);
            _stand.Cyl(0, new Vector3(HiHat.x, 0f, HiHat.z), HiHat + new Vector3(0f, -0.1f, 0f), 0.06f);
            _stand.Cyl(1, new Vector3(Crash.x, 0f, Crash.z), Crash, 0.06f);
            _stand.Cyl(2, new Vector3(Snare.x, 0f, Snare.z), Snare + new Vector3(0f, -0.35f, 0f), 0.07f);
            // The stool.
            _shell.Cyl(4, new Vector3(0f, 0f, 14f), new Vector3(0f, 2.55f, 14f), 0.1f);
            _shell.Cyl(5, new Vector3(0f, 2.55f, 14f), new Vector3(0f, 2.75f, 14f), 0.75f);
            _shell.Hide(6); _skin.Hide(4); _skin.Hide(5); _skin.Hide(6);
            // A riser under the kit.
            _dark.Box(0, new Vector3(0f, 0.12f, 12.8f), Quaternion.identity, new Vector3(8f, 0.24f, 6f));
            _neon.Box(0, new Vector3(0f, 0.28f, 9.75f), Quaternion.identity, new Vector3(8f, 0.06f, 0.06f));
            _neon.Glow(0, hue + 0.2f, 0.4f + 1.4f * rx.lum);
        }

        void DrawBass(float b, float play, float hue, float gain)
        {
            var origin = new Vector3(-9f, 0f, 8f);
            float nod = Mathf.Cos(b * Mathf.PI * 2f);
            var P = origin + new Vector3(0.3f * Mathf.Sin(b * Mathf.PI * 0.5f) * play, 0.51f * H - 0.05f * H * Mathf.Abs(Mathf.Cos(b * Mathf.PI)) * play, 0f);
            // The bass guitar hangs at the hip, neck pointing to the android's left (world +x), up a little.
            Vector3 body = P + new Vector3(0f, 0.04f * H, -0.12f * H);
            Vector3 neckEnd = body + new Vector3(3.4f, 1.0f, -0.4f);
            int note = ((int)Mathf.Floor(b / 2f)) & 3;
            float[] fret = { 0.38f, 0.62f, 0.5f, 0.82f };
            float fx = Mathf.Lerp(fret[(note + 3) & 3], fret[note], Mathf.SmoothStep(0f, 1f, (b / 2f - Mathf.Floor(b / 2f)) * 6f));
            Vector3 fretHand = Vector3.Lerp(body + new Vector3(0.6f, 0.15f, -0.2f), neckEnd, fx) + new Vector3(0f, 0.25f, -0.35f);
            float pluck = Strike(b, 0.5f) * play;
            Vector3 pluckHand = body + new Vector3(-0.55f, 0.05f + 0.22f * pluck, -0.55f);
            var pose = new AndroidPose
            {
                pelvis = P, yaw = -20f,
                chest = new Vector3(8f, 10f * play, 4f * Mathf.Sin(b * Mathf.PI * 0.5f)),
                head = new Vector3(8f * nod * play - 4f, 15f * Mathf.Sin(b * Mathf.PI * 0.25f), 0f),
                handL = Vector3.Lerp(body + new Vector3(0.2f, -0.6f, -0.3f), fretHand, play),
                handR = Vector3.Lerp(body + new Vector3(-0.4f, -0.7f, -0.3f), pluckHand, play),
                footL = origin + new Vector3(0.55f, 0.36f, 0f), footR = origin + new Vector3(-0.55f, 0.36f + 0.25f * Strike(b, 1f) * play, 0f),
                eye = 0.4f + 0.6f * rx.Band(0), core = 0.3f + 0.9f * rx.Band(0),
            };
            _bass.Apply(pose, hue + 0.2f, rx);
            // Guitar body (ellipsoid) and neck (a thin box), strings glow with the eased mids.
            _dark.Ellipsoid(1, body, Quaternion.AngleAxis(-15f, Vector3.forward), new Vector3(1.9f, 2.6f, 0.45f));
            Vector3 nd = (neckEnd - body);
            _dark.Box(2, body + nd * 0.5f, Quaternion.FromToRotation(Vector3.right, nd.normalized), new Vector3(nd.magnitude, 0.22f, 0.12f));
            _neon.Box(1, body + nd * 0.5f + new Vector3(0f, 0.13f, -0.08f), Quaternion.FromToRotation(Vector3.right, nd.normalized), new Vector3(nd.magnitude, 0.03f, 0.02f));
            _neon.Glow(1, hue + 0.25f, 0.5f + 2.5f * rx.midFast * play);
        }

        void DrawKeys(float b, float play, float hue, float gain)
        {
            var origin = new Vector3(9f, 0f, 8f);
            float nod = Mathf.Cos(b * Mathf.PI * 2f);
            var P = origin + new Vector3(0f, 0.51f * H - 0.04f * H * Mathf.Abs(Mathf.Cos(b * Mathf.PI)) * play, 0f);
            // An arpeggio: a key every half beat, stepping through a pattern that changes each bar.
            int[] pat = { 0, 4, 7, 12, 7, 4, 9, 14 };
            int step = ((int)Mathf.Floor(b * 2f)) & 7;
            int bar = ((int)Mathf.Floor(b / 4f)) & 3;
            int kn = Mathf.Clamp(2 + pat[step] / 2 + (bar == 2 ? 2 : 0), 0, Keys - 1);
            int kn2 = Mathf.Clamp(Keys - 3 - (pat[(step + 3) & 7] / 3), 0, Keys - 1);
            float kz = P.z - 1.9f;
            float PressA = Strike(b, 0.5f), PressB = Strike(b, 1f, 0.5f);
            Vector3 hA = new Vector3(origin.x + (kn - 7.5f) * 0.28f, 3.95f + 0.45f * PressA * play + 0.15f * (1f - play), kz);
            Vector3 hB = new Vector3(origin.x + (kn2 - 7.5f) * 0.28f, 3.95f + 0.5f * PressB * play + 0.15f * (1f - play), kz - 0.1f);
            var pose = new AndroidPose
            {
                pelvis = P, yaw = 0f,
                chest = new Vector3(14f, 8f * Mathf.Sin(b * Mathf.PI * 0.25f), 3f * Mathf.Sin(b * Mathf.PI * 0.5f)),
                head = new Vector3(6f * nod * play + 3f, 14f * Mathf.Sin(b * Mathf.PI * 0.125f), 0f),
                handL = hB, handR = hA,   // the android's left is world +x
                footL = origin + new Vector3(0.55f, 0.36f, 0f), footR = origin + new Vector3(-0.55f, 0.36f, 0f),
                eye = 0.4f + 0.6f * rx.Band(1), core = 0.3f + 0.9f * rx.Band(1),
            };
            _keys.Apply(pose, hue + 0.4f, rx);
            // The keyboard: 16 keys, the pressed ones lit; a stand under it.
            _dark.Box(3, new Vector3(origin.x, 3.55f, kz), Quaternion.identity, new Vector3(5.0f, 0.4f, 1.5f));
            for (int i = 0; i < Keys; i++)
            {
                Vector3 kp = new Vector3(origin.x + (i - 7.5f) * 0.28f, 3.8f, kz + 0.05f);
                bool down = (i == kn && PressA < 0.35f) || (i == kn2 && PressB < 0.35f);
                _key.Box(i, kp - new Vector3(0f, down ? 0.04f : 0f, 0f), Quaternion.identity, new Vector3(0.24f, 0.06f, 1.1f));
                float lit = (i == kn ? Hit(b, 0.5f, 0f, 6f) : 0f) + (i == kn2 ? Hit(b, 1f, 0.5f, 6f) : 0f);
                _keyTop.Box(i, kp + new Vector3(0f, 0.045f, 0.45f), Quaternion.identity, new Vector3(0.22f, 0.03f, 0.2f));
                _keyTop.Glow(i, hue + 0.04f * i, 0.1f + 3.5f * lit * play + 0.8f * rx.Spec(i / (Keys - 1f)));
            }
            _stand.Cyl(3, new Vector3(origin.x - 2f, 0f, kz), new Vector3(origin.x - 2f, 3.4f, kz), 0.08f);
            _stand.Cyl(4, new Vector3(origin.x + 2f, 0f, kz), new Vector3(origin.x + 2f, 3.4f, kz), 0.08f);
            _dark.Box(4, new Vector3(origin.x, 0.12f, 7.4f), Quaternion.identity, new Vector3(7f, 0.24f, 5f));
            _neon.Box(2, new Vector3(origin.x, 0.28f, 4.85f), Quaternion.identity, new Vector3(7f, 0.06f, 0.06f));
            _neon.Glow(2, hue + 0.4f, 0.4f + 1.4f * rx.lum);
            _dark.Box(5, new Vector3(-9f, 0.12f, 7.4f), Quaternion.identity, new Vector3(7f, 0.24f, 5f));
            _neon.Box(3, new Vector3(-9f, 0.28f, 4.85f), Quaternion.identity, new Vector3(7f, 0.06f, 0.06f));
            _neon.Glow(3, hue + 0.2f, 0.4f + 1.4f * rx.lum);
        }

        void DrawVox(float b, float play, float hue, float gain, float kick)
        {
            var origin = new Vector3(-3.2f, 0f, 3.5f);
            float nod = Mathf.Cos(b * Mathf.PI * 2f);
            var P = origin + new Vector3(0.4f * Mathf.Sin(b * Mathf.PI * 0.25f), 0.51f * H - 0.06f * H * Mathf.Abs(Mathf.Cos(b * Mathf.PI)), 0f);
            Vector3 head = P + new Vector3(0f, 0.43f * H, -0.2f);
            // Right hand on the mic stand at the mouth; left arm pumps every two beats, and goes up on a drop.
            Vector3 mic = head + new Vector3(-0.3f, -0.55f, -1.2f);
            float pump = Strike(b, 2f);
            Vector3 gest = new Vector3(origin.x + 2.2f, 4.4f + 2.6f * (1f - pump) * (0.5f + 0.5f * rx.midFast) * play, origin.z - 1.2f);
            float up = Mathf.Clamp01(rx.impact * 2.2f);
            Vector3 air = new Vector3(origin.x + 2.0f, 9.6f + 0.8f * Mathf.Sin(b * Mathf.PI), origin.z - 0.8f);
            Vector3 airR = new Vector3(origin.x - 2.4f, 9.4f + 0.8f * Mathf.Sin(b * Mathf.PI + Mathf.PI), origin.z - 0.8f);
            var pose = new AndroidPose
            {
                pelvis = P, yaw = 0f,
                chest = new Vector3(6f + 5f * kick, 8f * Mathf.Sin(b * Mathf.PI * 0.5f) * play, 5f * Mathf.Sin(b * Mathf.PI * 0.5f)),
                head = new Vector3(8f * nod * play - 5f, 18f * Mathf.Sin(b * Mathf.PI * 0.125f), 6f * Mathf.Sin(b * Mathf.PI * 0.25f)),
                handL = Vector3.Lerp(gest, air, up), handR = Vector3.Lerp(mic, airR, up),
                footL = origin + new Vector3(0.6f, 0.36f, 0f), footR = origin + new Vector3(-0.6f, 0.36f + 0.2f * Strike(b, 1f), 0.2f),
                eye = 0.4f + 0.6f * rx.Band(2), core = 0.3f + 0.9f * rx.Band(2) + 0.5f * rx.impact,
            };
            _vox.Apply(pose, hue + 0.6f, rx);
            // The mic stand and its glowing head.
            _stand.Cyl(5, new Vector3(mic.x, 0f, mic.z + 0.3f), mic + new Vector3(0f, -0.2f, 0.3f), 0.07f);
            _stand.Cyl(6, mic + new Vector3(0f, -0.2f, 0.3f), mic + new Vector3(0f, 0.1f, 0.05f), 0.16f);
            _stand.Hide(7);
            _dark.Box(6, new Vector3(-3.2f, 0.12f, 3.2f), Quaternion.identity, new Vector3(6f, 0.24f, 4f));
            _neon.Box(4, new Vector3(-3.2f, 0.28f, 1.15f), Quaternion.identity, new Vector3(6f, 0.06f, 0.06f));
            _neon.Glow(4, hue + 0.6f, 0.4f + 1.4f * rx.lum + 0.8f * rx.midFast);
            _dark.Hide(7); _neon.Hide(5);
        }
    }
}
