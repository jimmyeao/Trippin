// unity_djbot: a chrome-and-ceramic android DJ behind a pair of turntables and
// a mixer, in front of a wall of LEDs and a rig of lasers, lit like a club
// stage and reflected in the polished floor.
//  - Shape: the visor is a live spectrum display; the mixer's faders ride the
//    eased band levels and the VU lights follow them; the chest core swells
//    with the bass; the whole booth's edge lighting breathes with the loudness.
//  - Motion: the DJ nods on the kick, leans and twists with the phrase, and
//    works through a routine that changes every 4 bars: scratching a platter
//    (the record turns with the hand and reverses), riding the faders, both
//    hands on the platters; on a drop both arms shoot up (eased) and pump on
//    the energy clock; through a build one hand goes to the headphones.
//    Platters turn on the energy clock; lasers swing with the phrase.
//  - Luminance: the rig, screen and visor follow the music's loudness.
//  - Drops: a build dims and tightens the rig; the drop throws the arms up and
//    flares the screen (eased).
using UnityEngine;

namespace TrippinStage
{
    public class DjBotShow : KitShow
    {
        const float H = 9f;
        protected Transform root;   // the booth, DJ and rig live under this (the club show moves it back and scales it)
        RobotMats _mats;
        Android _dj;
        PartPool _halo, _haloDisc;
        PartPool _boxes, _deckBox, _platter, _rim, _marker, _fader, _knob, _knobMark, _led;
        Material _wall, _wallM;
        BeamPool _lasers, _shafts;
        // Eased routine weights: left hand {platter, mixer, air}, right hand {mixer, platter, air, phones}.
        readonly float[] _wl = new float[3], _wr = new float[4];
        float _spinL, _spinR;

        static readonly Quaternion Face = Quaternion.Euler(0f, 180f, 0f);

        /// Where the DJ stage is built: the show's own transform, or (robot club) a child placed deeper in the scene.
        protected virtual Transform MakeRoot() => transform;

        protected override void Build()
        {
            Env(160f, 0.06f);
            root = MakeRoot();
            _mats = new RobotMats(robotMat);
            _dj = new Android(root, _mats, H, Face, true, "dj", true);
            _halo = new PartPool(root, _mats.glow, PartPool.Kind.Cylinder, 1, "halo", false);
            _haloDisc = new PartPool(root, _mats.dark, PartPool.Kind.Cylinder, 1, "halo disc", false);
            _boxes = new PartPool(root, _mats.dark, PartPool.Kind.Cube, 4, "booth", true);
            _deckBox = new PartPool(root, _mats.chrome, PartPool.Kind.Cube, 3, "deck", true);
            _platter = new PartPool(root, _mats.dark, PartPool.Kind.Cylinder, 2, "platter", true);
            _rim = new PartPool(root, _mats.glow, PartPool.Kind.Cylinder, 2, "platter rim", true);
            _marker = new PartPool(root, _mats.glow, PartPool.Kind.Cube, 4, "marker", true);
            _fader = new PartPool(root, _mats.glow, PartPool.Kind.Cube, 5, "fader", true);
            _knob = new PartPool(root, _mats.chrome, PartPool.Kind.Cylinder, 8, "knob", true);
            _knobMark = new PartPool(root, _mats.glow, PartPool.Kind.Cube, 8, "knob mark", true);
            _led = new PartPool(root, _mats.glow, PartPool.Kind.Cube, 18, "vu", true);
            _wallM = Kit.Part(root, "led wall", Kit.GridMesh(1, 1, "wall"), new Material(screenMat), new Vector3(0f, 11f, 14f), new Vector3(46f, 22f, 1f))
                .GetComponent<Renderer>().sharedMaterial;
            _wallM.SetFloat("_Mode", 0f);
            _wallM.SetFloat("_Cols", 92f);
            _wallM.SetFloat("_Rows", 44f);
            _lasers = new BeamPool(root, beamMat, 14, "laser", false, 0.05f, 0f, 40f, 0.45f, 0.98f, 0.3f);
            _shafts = new BeamPool(root, beamMat, 6, "shaft", false, 0.12f, 0.004f, 8f, 0.7f, 0.4f, 0f);
        }

        static float Ease(float cur, float target, float dt) => Eased.Follow(cur, target, 3f, 3f, dt);

        protected override void Frame(ShowState s, float dt)
        {
            float gain = rx.Gain(0.6f, 1.45f);
            float hue = 0.5f + 0.08f * Mathf.Sin(rx.phrase * 0.5f);
            _mats.Tune(hue, gain);
            float kick = rx.kick, bass = rx.bassFast;

            // Routine: changes every 4 bars (16 beats).
            int stage = ((int)(rx.beat / 16f)) & 3;
            float[] tl = { stage == 1 ? 0f : 1f, stage == 1 ? 1f : 0f, 0f };
            float[] tr = { stage == 0 || stage == 1 ? 1f : 0f, stage == 3 ? 1f : 0f, stage == 2 ? 1f : 0f, 0f };
            float air = Mathf.Clamp01(rx.impact * 2.2f);
            float phones = Mathf.Clamp01((rx.tension - 0.45f) * 3f) * (1f - air);
            for (int i = 0; i < 3; i++) _wl[i] = Ease(_wl[i], tl[i] * (1f - air) + (i == 2 ? air : 0f), dt);
            for (int i = 0; i < 4; i++) _wr[i] = Ease(_wr[i], i == 3 ? phones : tr[i] * (1f - air) * (1f - phones) + (i == 2 ? air : 0f), dt);

            // Body.
            var P = new Vector3(0.25f * Mathf.Sin(rx.clk * 0.18f), 0.51f * H - 0.14f * kick, 4.9f);
            Vector3 chestAng = new Vector3(20f + 5f * bass, 12f * Mathf.Sin(rx.phrase), 4f * Mathf.Sin(rx.clk * 0.3f));
            Quaternion chestQ = Face * Quaternion.Euler(chestAng.x, chestAng.y, chestAng.z);
            Vector3 headPos = P + chestQ * new Vector3(0f, 0.4f * H, 0f);

            // Hand targets.
            // The android faces -z, so its left side is world +x.
            Vector3 cL = new Vector3(3.0f, 5.15f, 1.7f), cR = new Vector3(-3.0f, 5.15f, 1.7f);
            float scrL = 0.55f * Mathf.Sin(rx.clk * 0.37f) * (0.6f + rx.midFast) + 0.25f * Mathf.Sin(rx.clk * 0.91f);
            float scrR = 0.5f * Mathf.Sin(rx.clk * 0.41f + 1f) * (0.6f + rx.midFast) + 0.2f * Mathf.Sin(rx.clk * 0.77f);
            float phiL = 3.6f + scrL, phiR = -0.5f + scrR;
            Vector3 platL = cL + new Vector3(Mathf.Sin(phiL) * 0.75f, 0.08f + 0.06f * kick, Mathf.Cos(phiL) * 0.75f - 0.15f);
            Vector3 platR = cR + new Vector3(Mathf.Sin(phiR) * 0.75f, 0.08f + 0.06f * kick, Mathf.Cos(phiR) * 0.75f - 0.15f);
            Vector3 mixL = new Vector3(0.85f + 0.2f * Mathf.Sin(rx.clk * 0.5f), 5.2f, 1.0f + 0.3f * Mathf.Sin(rx.clk * 0.33f));
            float cross = Mathf.Sin(rx.clk * 0.31f) * 0.9f;
            Vector3 mixR = new Vector3(cross, 5.2f, 0.55f);
            float pump = 0.5f + 0.5f * Mathf.Sin(rx.clk * 0.9f);
            Vector3 airL = new Vector3(2.6f + 0.6f * Mathf.Sin(rx.clk * 0.45f), 10.4f + 1.4f * pump, 2.4f);
            Vector3 airR = new Vector3(-2.6f + 0.6f * Mathf.Sin(rx.clk * 0.45f + 1f), 10.4f + 1.4f * (1f - pump), 2.4f);
            Vector3 phoneR = headPos + chestQ * new Vector3(0.62f, -0.15f, 0.05f);
            float sl = _wl[0] + _wl[1] + _wl[2] + 1e-4f, sr = _wr[0] + _wr[1] + _wr[2] + _wr[3] + 1e-4f;
            Vector3 handL = (platL * _wl[0] + mixL * _wl[1] + airL * _wl[2]) / sl;
            Vector3 handR = (mixR * _wr[0] + platR * _wr[1] + airR * _wr[2] + phoneR * _wr[3]) / sr;

            var pose = new AndroidPose
            {
                pelvis = P,
                yaw = 0f,
                chest = chestAng,
                head = new Vector3(8f * kick + 4f * Mathf.Sin(rx.clk * 0.6f) - 6f, 24f * Mathf.Sin(rx.phrase * 0.5f), 5f * Mathf.Sin(rx.clk * 0.21f)),
                handL = handL,
                handR = handR,
                footL = new Vector3(-0.55f, 0.36f, P.z + 0.1f),
                footR = new Vector3(0.55f, 0.36f, P.z + 0.1f),
                eye = 0.4f + 0.6f * rx.lum,
                core = 0.3f + 0.7f * bass + 0.5f * rx.impact,
            };
            _dj.Apply(pose, hue, rx);

            // Booth, decks, mixer.
            var q0 = Quaternion.identity;
            _boxes.Box(0, new Vector3(0f, 2.3f, 0.15f), q0, new Vector3(11.4f, 4.6f, 0.3f));       // front panel
            _boxes.Box(1, new Vector3(0f, 4.5f, 1.75f), q0, new Vector3(11.4f, 0.2f, 3.1f));        // table top
            _boxes.Box(2, new Vector3(-5.6f, 2.3f, 1.7f), q0, new Vector3(0.3f, 4.6f, 3.1f));       // end panels
            _boxes.Box(3, new Vector3(5.6f, 2.3f, 1.7f), q0, new Vector3(0.3f, 4.6f, 3.1f));
            _deckBox.Box(0, new Vector3(-3.0f, 4.72f, 1.7f), q0, new Vector3(3.1f, 0.26f, 2.8f));
            _deckBox.Box(1, new Vector3(3.0f, 4.72f, 1.7f), q0, new Vector3(3.1f, 0.26f, 2.8f));
            _deckBox.Box(2, new Vector3(0f, 4.72f, 1.7f), q0, new Vector3(2.4f, 0.26f, 2.8f));      // mixer body
            // Edge strip along the booth front.
            _marker.Box(2, new Vector3(0f, 4.6f, 0.0f), q0, new Vector3(11.4f, 0.1f, 0.06f));
            _marker.Glow(2, hue + 0.1f, 0.4f + 1.4f * rx.lum);
            _marker.Box(3, new Vector3(0f, 0.15f, -0.02f), q0, new Vector3(11.4f, 0.08f, 0.06f));
            _marker.Glow(3, hue + 0.3f, 0.3f + 1.2f * rx.lum);

            // Platters: they turn on the energy clock, and follow the hand while it scratches.
            
            float angL = Mathf.Lerp(rx.clk * 1.2f, scrL * 2.2f, _wl[0]);
            float angR = Mathf.Lerp(-rx.clk * 1.0f, scrR * 2.2f, _wr[1]);
            for (int d = 0; d < 2; d++)
            {
                Vector3 c = d == 0 ? cL : cR;
                float ang = d == 0 ? angL : angR;
                _platter.Cyl(d, new Vector3(c.x, 4.88f, c.z), new Vector3(c.x, 5.06f, c.z), 1.15f);
                _rim.Cyl(d, new Vector3(c.x, 4.9f, c.z), new Vector3(c.x, 5.0f, c.z), 1.22f);
                _rim.Glow(d, hue + 0.15f * d, 0.35f + 1.2f * rx.lum);
                Quaternion mq = Quaternion.AngleAxis(ang * Mathf.Rad2Deg, Vector3.up);
                _marker.Box(d, new Vector3(c.x, 5.08f, c.z) + mq * new Vector3(0.55f, 0f, 0f), mq, new Vector3(0.95f, 0.025f, 0.07f));
                _marker.Glow(d, hue + 0.05f + 0.2f * d, 1.2f + 1.5f * rx.lum);
            }
            // Mixer: four channel faders ride the bands, a crossfader, knobs and VU lights.
            for (int i = 0; i < 4; i++)
            {
                float lvl = rx.Band(i);
                float x = -0.8f + 0.533f * i;
                _fader.Box(i, new Vector3(x, 4.92f, 1.55f + (lvl - 0.5f) * 1.1f), q0, new Vector3(0.2f, 0.07f, 0.34f));
                _fader.Glow(i, hue + 0.12f * i, 0.8f + 2f * lvl);
            }
            _fader.Box(4, new Vector3(cross, 4.92f, 0.55f), q0, new Vector3(0.36f, 0.07f, 0.2f));
            _fader.Glow(4, hue + 0.5f, 1.5f);
            for (int i = 0; i < 8; i++)
            {
                float x = -0.85f + 0.243f * i;
                Vector3 kp = new Vector3(x, 4.92f, 2.55f);
                _knob.Cyl(i, kp, kp + new Vector3(0f, 0.12f, 0f), 0.1f);
                Quaternion kq = Quaternion.AngleAxis((rx.clk * 8f * (0.5f + 0.1f * i) + i * 40f) % 360f, Vector3.up);
                _knobMark.Box(i, kp + new Vector3(0f, 0.13f, 0f) + kq * new Vector3(0.05f, 0f, 0f), kq, new Vector3(0.1f, 0.02f, 0.03f));
                _knobMark.Glow(i, hue + 0.05f * i, 1.2f + rx.Spec(i / 7f) * 2f);
            }
            for (int i = 0; i < 9; i++)
            {
                float lv = rx.Spec(i / 8f);
                for (int sd = 0; sd < 2; sd++)
                {
                    int k = i * 2 + sd;
                    float x = sd == 0 ? -1.12f : 1.12f;
                    float on = Mathf.Clamp01((lv * 9f - (8 - i)) + 0.15f);
                    _led.Box(k, new Vector3(x, 4.9f, 0.7f + i * 0.2f), q0, new Vector3(0.12f, 0.05f, 0.12f));
                    _led.Glow(k, hue + (i < 6 ? 0.1f : (i < 8 ? 0.3f : 0.5f)), 0.05f + 2.2f * on);
                }
            }

            // A halo ring behind the DJ's head (a glowing disc with a dark disc in front of it).
            _halo.Cyl(0, new Vector3(0f, 8.0f, 7.4f), new Vector3(0f, 8.0f, 7.3f), 4.6f);
            _halo.Glow(0, hue + 0.1f, 0.6f + 1.6f * rx.lum + 0.8f * bass);
            _haloDisc.Cyl(0, new Vector3(0f, 8.0f, 7.3f), new Vector3(0f, 8.0f, 7.0f), 4.3f);

            // LED wall.
            _wallM.SetFloat("_Hue", hue + 0.05f);
            _wallM.SetFloat("_Gain", gain * 0.9f);
            _wallM.SetFloat("_Clk", rx.clk);

            // Lasers fanning up from the floor behind the DJ: a formation every 4 bars, swinging with the phrase.
            int form = ((int)(rx.beat / 16f)) % 3;
            for (int i = 0; i < 14; i++)
            {
                float u = (i - 6.5f) / 6.5f;
                var from = new Vector3(u * 15f, 0.4f, 10f);
                float ang;
                switch (form)
                {
                    case 0: ang = u * 0.9f + 0.5f * Mathf.Sin(rx.phrase); break;
                    case 1: ang = -u * 0.7f + 0.4f * Mathf.Sin(rx.phrase + i * 0.4f); break;
                    default: ang = 0.45f * Mathf.Sin(rx.phrase * 2f + i * 0.9f); break;
                }
                var to = from + new Vector3(Mathf.Sin(ang) * 22f, 26f, 8f * Mathf.Cos(ang));
                _lasers.Set(i, from, to, Kit.Hue(0.5f + 0.06f * (i % 5)), gain * (0.3f + 0.7f * rx.midFast + 0.4f * rx.impact));
            }
            for (int i = 0; i < 6; i++)
            {
                float x = (i - 2.5f) * 6f;
                _shafts.Set(i, new Vector3(x, 0f, 12f), new Vector3(x + 5f * Mathf.Sin(rx.phrase + i), 30f, 8f), Kit.Hue(0.5f + 0.08f * i), gain * (0.05f + 0.14f * rx.Spec(i / 5f)));
            }

            // Camera: low in front, drifting, pushing in on a build.
            Vector3 pos, look;
            CameraPose(out pos, out look);
            rig.Move(cam, pos, look, dt, 0.7f);
        }

        /// The camera's target pose (the club show shoots from behind the crowd).
        protected virtual void CameraPose(out Vector3 pos, out Vector3 look)
        {
            float sway = Mathf.Sin(rx.phrase * 0.5f);
            pos = new Vector3(sway * 5f, 6.2f + 0.5f * Mathf.Sin(rx.phrase), -5.5f + 2f * rx.tension);
            look = new Vector3(0f, 7.0f, 4f);
        }
    }
}
