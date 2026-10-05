// One DJ station for the robot shows: a chrome-and-ceramic android behind two
// turntables and a mixer, with a halo ring, built under `root` in its own local
// frame (the android faces -z), so a show can place and rotate it as it likes:
// unity_djbot has one, unity_dj_battle has two facing each other. All motion
// rides the beat (Rx.beatS) and the eased band levels; `activity` (0..1) scales
// how hard the DJ is working (a DJ who is listening nods and keeps both hands
// on the records, but doesn't scratch).
using UnityEngine;

namespace TrippinStage
{
    public sealed class DjStation
    {
        const float H = 9f;
        static readonly Quaternion Face = Quaternion.Euler(0f, 180f, 0f);
        readonly Android _dj;
        readonly PartPool _halo, _haloDisc;
        readonly PartPool _boxes, _deckBox, _platter, _rim, _marker, _fader, _knob, _knobMark, _led;
        // Eased routine weights: left hand {platter, mixer, air}, right hand {mixer, platter, air, phones}.
        readonly float[] _wl = new float[3], _wr = new float[4];
        float _prevBeat = -1f, _angL, _angR;

        public DjStation(Transform root, RobotMats mats, string name)
        {
            _dj = new Android(root, mats, H, Face, true, name, true);
            _halo = new PartPool(root, mats.glow, PartPool.Kind.Cylinder, 1, name + " halo", false);
            _haloDisc = new PartPool(root, mats.dark, PartPool.Kind.Cylinder, 1, name + " halo disc", false);
            _boxes = new PartPool(root, mats.dark, PartPool.Kind.Cube, 4, name + " booth", true);
            _deckBox = new PartPool(root, mats.chrome, PartPool.Kind.Cube, 3, name + " deck", true);
            _platter = new PartPool(root, mats.dark, PartPool.Kind.Cylinder, 2, name + " platter", true);
            _rim = new PartPool(root, mats.glow, PartPool.Kind.Cylinder, 2, name + " platter rim", true);
            _marker = new PartPool(root, mats.glow, PartPool.Kind.Cube, 4, name + " marker", true);
            _fader = new PartPool(root, mats.glow, PartPool.Kind.Cube, 5, name + " fader", true);
            _knob = new PartPool(root, mats.chrome, PartPool.Kind.Cylinder, 8, name + " knob", true);
            _knobMark = new PartPool(root, mats.glow, PartPool.Kind.Cube, 8, name + " knob mark", true);
            _led = new PartPool(root, mats.glow, PartPool.Kind.Cube, 18, name + " vu", true);
        }

        // The same angle (mod 2 pi) as `a`, within half a turn of `target`: the platter brakes the short way round.
        static float Short(float a, float target) => target + Mathf.Repeat(a - target + Mathf.PI, Mathf.PI * 2f) - Mathf.PI;

        static float Ease(float cur, float target, float dt) => Eased.Follow(cur, target, 3f, 3f, dt);

        public void Update(Rx rx, float hue, float gain, float dt, float activity = 1f)
        {
            float kick = rx.kick, bass = rx.bassFast;
            // Routine: changes every 4 bars (16 beats).
            int stage = activity < 0.5f ? 3 : ((int)(rx.beat / 16f)) & 3;      // a DJ who is listening keeps both hands on the records
            float[] tl = { stage == 1 ? 0f : 1f, stage == 1 ? 1f : 0f, 0f };
            float[] tr = { stage == 0 || stage == 1 ? 1f : 0f, stage == 3 ? 1f : 0f, stage == 2 ? 1f : 0f, 0f };
            float air = Mathf.Clamp01(rx.impact * 2.2f);
            float phones = Mathf.Clamp01((rx.tension - 0.45f) * 3f) * (1f - air);
            for (int i = 0; i < 3; i++) _wl[i] = Ease(_wl[i], tl[i] * (1f - air) + (i == 2 ? air : 0f), dt);
            for (int i = 0; i < 4; i++) _wr[i] = Ease(_wr[i], i == 3 ? phones : tr[i] * (1f - air) * (1f - phones) + (i == 2 ? air : 0f), dt);

            // Body.
            var P = new Vector3(0.25f * Mathf.Sin(rx.beatS * Mathf.PI * 0.25f), 0.51f * H - 0.14f * kick - 0.05f * Mathf.Abs(Mathf.Cos(rx.beatS * Mathf.PI)), 4.9f);
            Vector3 chestAng = new Vector3(20f + 5f * bass, 12f * Mathf.Sin(rx.phrase), 4f * Mathf.Sin(rx.beatS * Mathf.PI * 0.25f));
            Quaternion chestQ = Face * Quaternion.Euler(chestAng.x, chestAng.y, chestAng.z);
            Vector3 headPos = P + chestQ * new Vector3(0f, 0.4f * H, 0f);

            // Hand targets.
            // The android faces -z, so its left side is world +x.
            Vector3 cL = new Vector3(3.0f, 5.15f, 1.7f), cR = new Vector3(-3.0f, 5.15f, 1.7f);
            // Scratches are strokes on the beat: a forward-back every two beats plus a flare on the half beats;
            // the right hand answers a beat later.
            float bt = rx.beatS * Mathf.PI;
            float scrL = activity * (0.55f * Mathf.Sin(bt) * (0.6f + rx.midFast) + 0.25f * Mathf.Sin(bt * 4f));
            float scrR = activity * (0.5f * Mathf.Sin(bt + Mathf.PI * 0.5f) * (0.6f + rx.midFast) + 0.2f * Mathf.Sin(bt * 2f));
            float phiL = 3.6f + scrL, phiR = -0.5f + scrR;
            Vector3 platL = cL + new Vector3(Mathf.Sin(phiL) * 0.75f, 0.08f + 0.06f * kick, Mathf.Cos(phiL) * 0.75f - 0.15f);
            Vector3 platR = cR + new Vector3(Mathf.Sin(phiR) * 0.75f, 0.08f + 0.06f * kick, Mathf.Cos(phiR) * 0.75f - 0.15f);
            Vector3 mixL = new Vector3(0.85f + 0.2f * Mathf.Sin(rx.beatS * Mathf.PI * 0.5f), 5.2f, 1.0f + 0.3f * Mathf.Sin(rx.beatS * Mathf.PI * 0.25f));
            float cross = Mathf.Sin(rx.beatS * Mathf.PI * 0.125f) * 0.9f;      // one slide per 16 beats
            Vector3 mixR = new Vector3(cross, 5.2f, 0.55f);
            float pump = 0.5f + 0.5f * Mathf.Sin(rx.beatS * Mathf.PI);          // up on a beat, down on the next
            Vector3 airL = new Vector3(2.6f + 0.6f * Mathf.Sin(rx.beatS * Mathf.PI * 0.5f), 10.4f + 1.4f * pump, 2.4f);
            Vector3 airR = new Vector3(-2.6f + 0.6f * Mathf.Sin(rx.beatS * Mathf.PI * 0.5f + Mathf.PI), 10.4f + 1.4f * (1f - pump), 2.4f);
            Vector3 phoneR = headPos + chestQ * new Vector3(0.62f, -0.15f, 0.05f);
            float sl = _wl[0] + _wl[1] + _wl[2] + 1e-4f, sr = _wr[0] + _wr[1] + _wr[2] + _wr[3] + 1e-4f;
            Vector3 handL = (platL * _wl[0] + mixL * _wl[1] + airL * _wl[2]) / sl;
            Vector3 handR = (mixR * _wr[0] + platR * _wr[1] + airR * _wr[2] + phoneR * _wr[3]) / sr;

            var pose = new AndroidPose
            {
                pelvis = P,
                yaw = 0f,
                chest = chestAng,
                head = new Vector3(8f * kick + 4f * Mathf.Cos(rx.beatS * Mathf.PI * 2f) - 6f, 24f * Mathf.Sin(rx.phrase * 0.5f), 5f * Mathf.Sin(rx.beatS * Mathf.PI * 0.25f)),
                handL = handL,
                handR = handR,
                footL = new Vector3(-0.55f, 0.36f, P.z + 0.1f),
                footR = new Vector3(0.55f, 0.36f, P.z + 0.1f),
                eye = (0.4f + 0.6f * rx.lum) * (0.5f + 0.5f * activity),
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
            
            // A platter turns once every four beats (about 31 rpm at 126 BPM, like a real deck).
            // Integrated from the beat advance (continuous even across a tempo correction): it turns with the
            // beat while the hand is off the record, and is pulled onto the hand's stroke while scratching.
            float dBeat = _prevBeat < 0f ? 0f : Mathf.Clamp(rx.beatS - _prevBeat, 0f, 0.5f);
            _prevBeat = rx.beatS;
            float pull = 1f - Mathf.Exp(-12f * dt);
            _angL += dBeat * Mathf.PI * 0.5f * (1f - _wl[0]);
            _angL = Short(_angL, scrL * 2.2f);
            _angL = Mathf.Lerp(_angL, scrL * 2.2f, _wl[0] * pull);
            _angR -= dBeat * Mathf.PI * 0.5f * (1f - _wr[1]);
            _angR = Short(_angR, scrR * 2.2f);
            _angR = Mathf.Lerp(_angR, scrR * 2.2f, _wr[1] * pull);
            float angL = _angL, angR = _angR;
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
            _halo.Glow(0, hue + 0.1f, (0.6f + 1.6f * rx.lum + 0.8f * bass) * (0.3f + 0.7f * activity));
            _haloDisc.Cyl(0, new Vector3(0f, 8.0f, 7.3f), new Vector3(0f, 8.0f, 7.0f), 4.3f);

        }
    }
}
