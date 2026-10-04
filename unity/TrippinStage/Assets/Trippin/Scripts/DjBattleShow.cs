// unity_dj_battle: two android DJs side by side in front of an LED wall and a
// laser fan, trading the decks every four bars. The one who has the floor
// scratches, rides the faders and holds the spotlight and the louder halo; the
// other nods along with both hands resting on the records, then takes over.
// On a drop both go all out with their arms up.
//  - Shape: each visor is a live spectrum, each mixer's faders ride the bands, each
//    tower of lights over a booth fills with the spectrum (louder for the DJ who
//    has the floor), chest cores swell with the bass.
//  - Motion: everything rides the beat (Rx.beatS): scratch strokes, nods, pumps,
//    the platters (one turn every four beats); the hand-over every 16 beats is
//    eased, so the DJs trade without a snap; the camera leans toward whoever
//    has the floor and swings with the phrase.
//  - Luminance: booths, halos, wall and spotlights follow the music's loudness.
//  - Drops: a build tightens the rig and dims it; the drop brings both DJs to full
//    activity with arms up (eased) and flares the wall.
using UnityEngine;

namespace TrippinStage
{
    public sealed class DjBattleShow : KitShow
    {
        const int Tower = 10;
        RobotMats _mats;
        readonly DjStation[] _st = new DjStation[2];
        readonly Transform[] _root = new Transform[2];
        PartPool _tower;
        Material _wallM;
        BeamPool _lasers, _spots;
        float _act;   // 0 = A has the floor, 1 = B

        static readonly Vector3[] Pos = { new Vector3(-9f, 0f, 0f), new Vector3(9f, 0f, 0f) };

        protected override void Build()
        {
            Env(160f, 0.06f);
            _mats = new RobotMats(robotMat);
            for (int i = 0; i < 2; i++)
            {
                var g = new GameObject("station " + (i == 0 ? "A" : "B"));
                g.transform.SetParent(transform, false);
                g.transform.localPosition = Pos[i];
                _root[i] = g.transform;
                _st[i] = new DjStation(_root[i], _mats, i == 0 ? "djA" : "djB");
            }
            _tower = new PartPool(transform, _mats.glow, PartPool.Kind.Cube, 2 * Tower, "tower", true);
            _wallM = Kit.Part(transform, "led wall", Kit.GridMesh(1, 1, "wall"), new Material(screenMat), new Vector3(0f, 12f, 16f), new Vector3(60f, 24f, 1f))
                .GetComponent<Renderer>().sharedMaterial;
            _wallM.SetFloat("_Mode", 0f);
            _wallM.SetFloat("_Cols", 100f);
            _wallM.SetFloat("_Rows", 40f);
            _lasers = new BeamPool(transform, beamMat, 14, "laser", false, 0.05f, 0f, 40f, 0.45f, 0.98f, 0.3f);
            _spots = new BeamPool(transform, beamMat, 2, "spot", false, 0.16f, 0.014f, 8f, 0.65f, 0.55f, 0f);
        }

        static float FormAng(int form, float u, int i, float phrase) =>
            form == 0 ? u * 0.9f + 0.5f * Mathf.Sin(phrase) : (form == 1 ? -u * 0.7f + 0.4f * Mathf.Sin(phrase + i * 0.4f) : 0.45f * Mathf.Sin(phrase * 2f + i * 0.9f));

        protected override void Frame(ShowState s, float dt)
        {
            float gain = rx.Gain(0.6f, 1.45f);
            float hue = 0.5f + 0.08f * Mathf.Sin(rx.phrase * 0.5f);
            _mats.Tune(hue, gain);
            // Who has the floor: A for 16 beats, then B; both on a drop.
            float target = ((int)(rx.beatS / 16f) & 1) == 0 ? 0f : 1f;
            _act = Eased.Follow(_act, target, 2f, 2f, dt);
            float both = Mathf.Clamp01(rx.impact * 2.2f);
            float actA = Mathf.Max(1f - _act, both), actB = Mathf.Max(_act, both);
            _st[0].Update(rx, hue, gain, dt, actA);
            _st[1].Update(rx, hue + 0.18f, gain, dt, actB);

            // Towers of lights over the booths: filled by the spectrum, brighter for the active DJ.
            var q0 = Quaternion.identity;
            for (int d = 0; d < 2; d++)
            {
                float act = d == 0 ? actA : actB;
                for (int i = 0; i < Tower; i++)
                {
                    float lv = rx.Spec((d == 0 ? i : Tower - 1 - i) / (Tower - 1f) * 0.9f);
                    float on = Mathf.Clamp01(lv * Tower - (Tower - 1 - i) + 0.35f) * (0.35f + 0.65f * act);
                    int k = d * Tower + i;
                    _tower.Box(k, Pos[d] + new Vector3(0f, 13f + i * 0.9f, 7.5f), q0, new Vector3(3.2f - 0.12f * i, 0.5f, 0.3f));
                    _tower.Glow(k, hue + 0.18f * d + 0.03f * i, 0.05f + 2.4f * on);
                }
                // A spotlight on the DJ who has the floor.
                _spots.Set(d, Pos[d] + new Vector3(0f, 30f, 6f), Pos[d] + new Vector3(0f, 8f, 4.9f), Kit.Hue(hue + 0.18f * d), gain * (0.03f + 0.3f * act));
            }

            _wallM.SetFloat("_Hue", hue + 0.1f);
            _wallM.SetFloat("_Gain", gain * 0.9f);
            _wallM.SetFloat("_Clk", rx.clk);
            // A new laser formation every 16 beats, morphing in over the first beat (a hard switch
            // read as a pop at every hand-over).
            int blk = Mathf.FloorToInt(rx.beatS / 16f);
            int form = ((blk % 3) + 3) % 3, prevForm = (form + 2) % 3;
            float morph = Mathf.SmoothStep(0f, 1f, Mathf.Clamp01(rx.beatS - blk * 16f));
            for (int i = 0; i < 14; i++)
            {
                float u = (i - 6.5f) / 6.5f;
                var from = new Vector3(u * 24f, 0.4f, 12f);
                float ang = Mathf.Lerp(FormAng(prevForm, u, i, rx.phrase), FormAng(form, u, i, rx.phrase), morph);
                _lasers.Set(i, from, from + new Vector3(Mathf.Sin(ang) * 22f, 28f, 8f * Mathf.Cos(ang)), Kit.Hue(hue + 0.06f * (i % 5) + (u > 0f ? 0.18f : 0f)), gain * (0.3f + 0.7f * rx.midFast + 0.4f * rx.impact));
            }

            // Camera: leans toward whoever has the floor, swings with the phrase, pushes in on a build.
            float sway = Mathf.Sin(rx.phrase * 0.5f);
            rig.Move(cam, new Vector3(sway * 5f + (_act - 0.5f) * 7f, 7.0f + 0.6f * Mathf.Sin(rx.phrase), -23f + 4f * rx.tension),
                new Vector3((_act - 0.5f) * 5f, 8.0f, 5f), dt, 0.7f);
        }
    }
}
