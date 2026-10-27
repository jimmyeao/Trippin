// unity_groovebox: close-ups of a classic 16-step drum machine on the booth
// table (faceplate in tools/gear_textures.py: eleven instrument columns with
// their knobs, tempo, volume and instrument select, START/STOP, 16 step keys
// in four colour groups with an LED over each, wooden end cheeks). It runs a
// pattern in time with the music:
//  - Motion: the running light chases across the 16 steps on the 16ths of
//    Trippin's beat (bar-aligned); the instrument select turns to the next
//    instrument every 4 bars and the step LEDs show that instrument's pattern;
//    the camera drifts through macro shots, cutting every 8 bars.
//  - Shape: a step key dips when its step fires for the selected instrument;
//    the bass drum decay follows the bass, the hat decays the highs, the
//    snare's snappy rides the build, the toms' tune drifts with the phrase,
//    and a breakdown pulls the bass drum level right down (back on the drop).
// Controls: drift, booth lamp, club lights, LED glow.
using UnityEngine;

namespace TrippinStage
{
    public sealed class GrooveboxShow : KitShow
    {
        public Material gearMat, clubMat;
        public Texture2D plateTex;

        [TrippinParam("Drift", 0f, 2f)] public float drift = 1f;
        [TrippinParam("Booth lamp", 0.3f, 2f)] public float lamp = 1f;
        [TrippinParam("Club lights", 0f, 2f)] public float club = 1f;
        [TrippinParam("LED glow", 0.3f, 2f)] public float leds = 1f;

        const float TopY = 0.72f;
        // Patterns (16 steps) for the eleven instruments, in column order.
        static readonly int[][] Pat =
        {
            new[] { 0, 4, 8, 12 },                 // bass drum
            new[] { 4, 12 },                       // snare
            new[] { 11 }, new[] { 13 }, new[] { 14, 15 },
            new[] { 3, 10 },                       // rim
            new[] { 4, 12 },                       // clap
            new[] { 0, 2, 4, 6, 8, 10, 12, 14 },   // closed hat
            new[] { 2, 6, 10, 14 },                // open hat
            new[] { 0 },                           // crash
            new[] { 1, 3, 5, 7, 9, 11, 13, 15 },   // ride
        };
        static readonly int[] SelectOrder = { 0, 1, 7, 8, 6, 2, 3, 4, 5, 9, 10 };
        static readonly int[] KnobCount = { 3, 3, 3, 3, 3, 1, 1, 2, 2, 2, 2 };
        static readonly Color[] KeyCol =
        {
            new Color(0.55f, 0.05f, 0.035f), new Color(0.8f, 0.33f, 0.04f), new Color(0.82f, 0.66f, 0.08f), new Color(0.74f, 0.72f, 0.64f),
        };

        GearMats _m;
        Booth _booth;
        Material _plate, _wood, _start, _key;
        Mesh _body, _cheek, _keyM, _ledM, _knobS, _knobL, _startM, _tapM;
        Vector3 _o;
        readonly float[] _press = new float[16];
        float _bd = 1f, _bass, _high, _sel;
        int _lastStep = -1, _shot = -1;

        Vector3 DM(float x, float z, float y = 0f) => _o + new Vector3((x - 250f) * 0.01f, y * 0.01f, (z - 150f) * 0.01f);

        protected override void Build()
        {
            _m = new GearMats(gearMat);
            _booth = new Booth(transform, clubMat, _m);
            _plate = _m.Tex(plateTex, new Color(0.035f, 0.035f, 0.038f), 0.3f, 0.45f);
            _wood = _m.M(0.16f, 0.075f, 0.035f, 0f, 0.42f, 0.3f);
            _start = _m.M(0.6f, 0.04f, 0.03f, 0f, 0.4f);
            _key = _m.M(0.5f, 0.5f, 0.5f, 0f, 0.42f);
            _body = GearMesh.RoundedBox(new Vector3(5.0f, 0.7f, 3.0f), 0.04f, 3, "drum machine");
            _cheek = GearMesh.RoundedBox(new Vector3(0.20f, 0.86f, 3.06f), 0.05f, 3, "wood cheek");
            _keyM = GearMesh.RoundedBox(new Vector3(0.22f, 0.09f, 0.34f), 0.03f, 3, "step key");
            _ledM = GearMesh.Lathe(new[] { new Vector2(0f, 0.035f), new Vector2(0.02f, 0.03f), new Vector2(0.025f, 0.015f), new Vector2(0.025f, 0.015f), new Vector2(0.025f, 0f) }, 16, "step led");
            _knobS = GearMesh.Knob(0.085f, 0.16f, "drum knob");
            _knobL = GearMesh.Knob(0.13f, 0.20f, "tempo knob");
            _startM = GearMesh.RoundedBox(new Vector3(0.34f, 0.10f, 0.34f), 0.04f, 3, "start stop");
            _tapM = GearMesh.RoundedBox(new Vector3(0.26f, 0.07f, 0.13f), 0.03f, 2, "tap");
            _o = new Vector3(0f, TopY, -0.4f);
        }

        static Matrix4x4 T(Vector3 p) => GearDraw.T(p);
        static Matrix4x4 TRS(Vector3 p, Quaternion q) => GearDraw.TRS(p, q);

        void Knob(Mesh mesh, Material mat, float r, float x, float z, float val, bool centred = false)
        {
            float a = centred ? val * 135f : -135f + 270f * Mathf.Clamp01(val);
            var pr = GearDraw.Props; pr.SetFloat("_Spin", r);
            GearDraw.Draw(mesh, mat, TRS(DM(x, z), Quaternion.Euler(0f, a, 0f)), pr);
        }

        static bool Hit(int inst, int step)
        {
            foreach (var p in Pat[inst]) if (p == step) return true;
            return false;
        }

        protected override void Frame(ShowState s, float dt)
        {
            dt = Mathf.Clamp(dt, 1e-4f, 1f / 20f);
            GearDraw.BeginFrame();
            _booth.Frame(rx, lamp, club);
            float b = rx.beatS;
            int step = ((Mathf.FloorToInt(b * 4f) % 16) + 16) % 16;
            int blk = Mathf.FloorToInt(b / 16f);
            int inst = SelectOrder[((blk % SelectOrder.Length) + SelectOrder.Length) % SelectOrder.Length];
            if (step != _lastStep)
            {
                _lastStep = step;
                if (Hit(inst, step)) _press[step] = 1f;
            }
            for (int i = 0; i < 16; i++) _press[i] = Mathf.Max(0f, _press[i] - dt / 0.12f);
            _bass = Eased.Follow(_bass, rx.bassFast, 14f, 3.5f, dt);
            _high = Eased.Follow(_high, rx.highFast, 14f, 3.5f, dt);
            _bd = Mathf.Lerp(_bd, rx.calm > 0.5f ? 0.08f : 0.85f, 1f - Mathf.Exp(-dt * 3f));
            // The instrument select turns to its new position over half a beat.
            _sel = Mathf.Lerp(_sel, inst / 10f, 1f - Mathf.Exp(-dt * 8f));

            GearDraw.Draw(_body, _plate, T(DM(250f, 150f, -35f)));
            GearDraw.Draw(_cheek, _wood, T(DM(-10f, 150f, -33f)));
            GearDraw.Draw(_cheek, _wood, T(DM(510f, 150f, -33f)));

            // Instrument columns.
            float phrase = Mathf.Sin(rx.phrase * 0.5f);
            for (int i = 0; i < 11; i++)
            {
                float x = 82f + i * 38f;
                for (int k = 0; k < KnobCount[i]; k++)
                {
                    float z = 228f - k * 46f;
                    float v = 0.6f;
                    if (k == 0) v = i == 0 ? _bd : 0.7f;
                    else if (i == 0 && k == 2) v = 0.35f + 0.5f * _bass;
                    else if (i == 1 && k == 2) v = 0.3f + 0.6f * rx.tension;
                    else if ((i == 7 || i == 8) && k == 1) v = 0.3f + 0.55f * _high;
                    else if (i >= 2 && i <= 4 && k == 1) v = 0.5f + 0.25f * phrase * (i - 3);
                    Knob(_knobS, _m.knobLight, 0.085f, x, z, v);
                }
            }
            // Left column: tempo (from the BPM), volume, instrument select.
            Knob(_knobL, _m.knob, 0.13f, 32f, 228f, Mathf.InverseLerp(60f, 200f, s.bpm));
            Knob(_knobS, _m.knob, 0.10f, 32f, 170f, 0.75f);
            Knob(_knobS, _m.knob, 0.10f, 32f, 120f, _sel);
            // START/STOP lit while running, TAP blinking on the beat.
            {
                var pr = GearDraw.Props; pr.SetColor("_Emit", new Color(2.5f, 0.15f, 0.08f) * 0.4f * leds);
                GearDraw.Draw(_startM, _start, T(DM(32f, 44f, 2f)), pr);
                pr = GearDraw.Props; pr.SetColor("_Emit", new Color(2.5f, 1.2f, 0.2f) * (Mathf.Repeat(b, 1f) < 0.2f ? 0.8f : 0.05f) * leds);
                GearDraw.Draw(_tapM, _m.led, T(DM(32f, 79f, 1f)), pr);
            }
            // Step keys and their LEDs: the selected instrument's pattern lit, the running light bright.
            for (int i = 0; i < 16; i++)
            {
                float x = 82f + i * 25.6f;
                var pr = GearDraw.Props; pr.SetColor("_Tint", KeyCol[i / 4]); pr.SetColor("_Side", KeyCol[i / 4] * 0.7f);
                GearDraw.Draw(_keyM, _key, T(DM(x, 39f, 3f - 3.5f * _press[i])), pr);
                bool hit = Hit(inst, i);
                float on = i == step ? 1f : hit ? 0.35f : 0.02f;
                pr = GearDraw.Props; pr.SetColor("_Emit", new Color(3.5f, 0.2f, 0.12f) * on * leds);
                pr.SetColor("_Tint", new Color(0.12f, 0.02f, 0.02f));
                GearDraw.Draw(_ledM, _m.led, T(DM(x, 66f)), pr);
            }

            // Shots: one per 8 bars.
            int sb = Mathf.FloorToInt(b / 32f);
            if (sb != _shot) { _shot = sb; rig.Snap(); }
            float u = Mathf.Repeat(b, 32f) / 32f;
            float sw = Mathf.Sin(u * Mathf.PI - Mathf.PI * 0.5f) * drift;
            Vector3 pos, look; float fstop = 5.6f;
            switch (((sb % 5) + 5) % 5)
            {
                default:
                case 0: look = DM(250f, 150f); pos = look + new Vector3(sw * 1.2f, 3.4f, -5.2f); fstop = 8f; break;
                case 1: look = DM(250f + 120f * sw, 50f); pos = look + new Vector3(-1.6f, 0.5f, -1.7f); break;
                case 2: look = DM(120f, 200f); pos = look + new Vector3(0.9f + 0.4f * sw, 1.3f, -1.6f); break;
                case 3: look = DM(50f, 150f); pos = look + new Vector3(1.0f + 0.3f * sw, 1.4f, -1.5f); break;
                case 4: look = DM(330f, 160f); pos = look + new Vector3(0.6f * sw, 3.0f, -1.4f); fstop = 8f; break;
            }
            rig.Move(cam, pos, look, dt, 3f);
            HqPost.Focus(Vector3.Distance(cam.transform.position, look), fstop, 50f);
        }
    }
}
