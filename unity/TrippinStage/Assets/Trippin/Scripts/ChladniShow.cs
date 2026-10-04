// unity_chladni: a vibrating plate. The plate's height field is a Chladni
// figure, and the glowing lines are its nodal lines, where the plate is
// still - the "sand" lines of the classic experiment.
//  - Shape: the figure is one of six vibration modes (n, m); a new mode comes
//    in every 4 bars, morphing over two beats, and the mode numbers drift
//    with the eased mids, so the pattern keeps reshaping; the displacement
//    follows the eased bass.
//  - Motion: the plate turns slowly on the smooth energy clock and swings
//    direction with the phrase; the camera orbits above it.
//  - Luminance: the nodal lines follow the music's loudness.
//  - Drops: a build stills the plate dim; the drop makes it ring (eased).
using UnityEngine;

namespace TrippinStage
{
    public sealed class ChladniShow : KitShow
    {
        static readonly float[,] Modes = { { 1f, 2f }, { 2f, 3f }, { 3f, 4f }, { 2f, 5f }, { 4f, 5f }, { 3f, 6f } };
        Material _m;
        Transform _plate;
        int _cur = -1, _prev;
        float _start;

        protected override void Build()
        {
            Mesh mesh = Kit.GridMesh(180, 180, "plate");
            _m = new Material(surfaceMat);
            _m.SetFloat("_Mode", 2f);
            _m.SetFloat("_SizeX", 44f);
            _m.SetFloat("_SizeZ", 44f);
            _m.SetFloat("_Grid", 0.08f);
            _m.SetFloat("_Fade", 500f);
            _m.SetFloat("_Fill", 0.5f);
            _plate = Kit.Part(transform, "plate", mesh, _m, new Vector3(0f, 3f, 0f), Vector3.one).transform;
            haze = new HazeSet(transform, hazeMat, new[] { new Vector3(0f, 8f, 30f) }, new[] { new Vector2(120f, 40f) }, new[] { 0.45f }, new[] { 0.05f });
        }

        protected override void Frame(ShowState s, float dt)
        {
            int f = (Mathf.FloorToInt(rx.beat / 4f) / 4) % Modes.GetLength(0);
            if (_cur < 0) { _cur = _prev = f; _start = rx.beat - 4f; }
            else if (f != _cur) { _prev = _cur; _cur = f; _start = rx.beat; }
            float m = Mathf.SmoothStep(0f, 1f, (rx.beat - _start) / 2f);
            float n = Mathf.Lerp(Modes[_prev, 0], Modes[_cur, 0], m) + 0.18f * rx.midFast;
            float mm = Mathf.Lerp(Modes[_prev, 1], Modes[_cur, 1], m) + 0.12f * rx.highFast;
            float gain = rx.Gain(0.35f, 1.6f);
            _m.SetFloat("_Amp", (3f + 3f * rx.bassFast + 4f * rx.impact) * (1f - 0.6f * rx.tension));
            _m.SetFloat("_Intensity", 0.8f * gain);
            _m.SetFloat("_Hue", 0.45f + 0.1f * Mathf.Sin(rx.phrase * 0.5f));
            _m.SetVector("_P0", new Vector4(n, mm, 0f, 0f));
            _plate.localRotation = Quaternion.Euler(0f, (rx.clk * 0.4f + 25f * Mathf.Sin(rx.phrase)), 0f);
            rig.Orbit(cam, rx, 34f, 26f, 2f, dt);
        }
    }
}
