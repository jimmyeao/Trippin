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
//    All of it is locked to the beat (rx.beatS): scratch strokes, the nod, the
//    arm pumps and the crossfader; a platter turns once every four beats. Lasers
//    swing with the phrase.
//  - Luminance: the rig, screen and visor follow the music's loudness.
//  - Drops: a build dims and tightens the rig; the drop throws the arms up and
//    flares the screen (eased).
using UnityEngine;

namespace TrippinStage
{
    public class DjBotShow : KitShow
    {
        protected Transform root;   // the booth, DJ and rig live under this (the club show moves it back and scales it)
        RobotMats _mats;
        DjStation _station;
        Material _wallM;
        BeamPool _lasers, _shafts;

        /// Where the DJ stage is built: the show's own transform, or (robot club) a child placed deeper in the scene.
        protected virtual Transform MakeRoot() => transform;

        protected override void Build()
        {
            Env(160f, 0.06f);
            root = MakeRoot();
            _mats = new RobotMats(robotMat);
            _station = new DjStation(root, _mats, "dj");
            _wallM = Kit.Part(root, "led wall", Kit.GridMesh(1, 1, "wall"), new Material(screenMat), new Vector3(0f, 11f, 14f), new Vector3(46f, 22f, 1f))
                .GetComponent<Renderer>().sharedMaterial;
            _wallM.SetFloat("_Mode", 0f);
            _wallM.SetFloat("_Cols", 92f);
            _wallM.SetFloat("_Rows", 44f);
            _lasers = new BeamPool(root, beamMat, 14, "laser", false, 0.05f, 0f, 40f, 0.45f, 0.98f, 0.3f);
            _shafts = new BeamPool(root, beamMat, 6, "shaft", false, 0.12f, 0.004f, 8f, 0.7f, 0.4f, 0f);
        }

        static float FormAng(int form, float u, int i, float phrase) =>
            form == 0 ? u * 0.9f + 0.5f * Mathf.Sin(phrase) : (form == 1 ? -u * 0.7f + 0.4f * Mathf.Sin(phrase + i * 0.4f) : 0.45f * Mathf.Sin(phrase * 2f + i * 0.9f));

        protected override void Frame(ShowState s, float dt)
        {
            float gain = rx.Gain(0.6f, 1.45f);
            float hue = 0.5f + 0.08f * Mathf.Sin(rx.phrase * 0.5f);
            _mats.Tune(hue, gain);
            _station.Update(rx, hue, gain, dt);

            // LED wall.
            _wallM.SetFloat("_Hue", hue + 0.05f);
            _wallM.SetFloat("_Gain", gain * 0.9f);
            _wallM.SetFloat("_Clk", rx.clk);

            // Lasers fanning up from the floor behind the DJ: a formation every 4 bars, swinging with the phrase.
            // The new formation morphs in over the first beat of its 4 bars (a hard switch popped).
            int blk = Mathf.FloorToInt(rx.beatS / 16f);
            int form = ((blk % 3) + 3) % 3, prevForm = (form + 2) % 3;
            float morph = Mathf.SmoothStep(0f, 1f, Mathf.Clamp01(rx.beatS - blk * 16f));
            for (int i = 0; i < 14; i++)
            {
                float u = (i - 6.5f) / 6.5f;
                var from = new Vector3(u * 15f, 0.4f, 10f);
                float ang = Mathf.Lerp(FormAng(prevForm, u, i, rx.phrase), FormAng(form, u, i, rx.phrase), morph);
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
