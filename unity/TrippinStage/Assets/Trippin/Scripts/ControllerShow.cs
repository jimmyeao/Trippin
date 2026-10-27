// unity_controller: the streamer / home-DJ setup: a two-deck all-in-one DJ
// controller on the table (faceplate in tools/gear_textures.py: per deck a
// jog with a lit edge and a centre display, 8 performance pads, PLAY/PAUSE,
// CUE, SYNC and a tempo slider, loop and beat FX up top; a two-channel mixer
// with EQ, filter, CUE, faders, meters and a crossfader in the middle) and a
// laptop on a stand behind it running the DJ software (two waveforms
// scrolling, the library). The same 32-bar mix as unity_cdj:
//  - Shape: faders cross with a bass swap, the filter rides the build, the
//    meters follow the music; the pads switch mode with the track: hot cues
//    (one fires on the drop), a roll stepping up the row as the build tightens,
//    a 4-beat loop held through breakdowns.
//  - Motion: the waveforms scroll at the tempo, the jog displays' playheads
//    turn, the tempo sliders sit where the tempo needs them; the camera cuts
//    on bar lines, following the mix.
// Controls: drift, booth lamp, club lights, screen glow, LED glow.
using UnityEngine;

namespace TrippinStage
{
    public sealed class ControllerShow : KitShow
    {
        public Material gearMat, clubMat;
        public Texture2D plateTex, uiTex;

        [TrippinParam("Drift", 0f, 2f)] public float drift = 1f;
        [TrippinParam("Booth lamp", 0.3f, 2f)] public float lamp = 1f;
        [TrippinParam("Club lights", 0f, 2f)] public float club = 1f;
        [TrippinParam("Screen glow", 0.3f, 2f)] public float glow = 1f;
        [TrippinParam("LED glow", 0.3f, 2f)] public float leds = 1f;

        const float TopY = 0.62f;
        static readonly Color[] PadCol =
        {
            new Color(0.1f, 1f, 0.2f), new Color(1f, 0.5f, 0.05f), new Color(0.1f, 0.4f, 1f), new Color(1f, 0.1f, 0.4f),
            new Color(0.1f, 1f, 0.2f), new Color(0.6f, 0.1f, 1f), new Color(1f, 0.85f, 0.1f), new Color(0.1f, 0.8f, 1f),
        };

        GearMats _m;
        Booth _booth;
        Material _plate, _ui, _jogTop, _jogRing, _jogDisp, _alu, _keys;
        Mesh _body, _foot, _jogTopM, _jogRingM, _jogGlow, _jogDispM, _pad, _bigBtn, _bigRing, _small, _knobS, _knobM, _cap, _capLine, _xfCap,
             _meterQ, _lapBase, _lapKeys, _lapPad, _lid, _screen, _stand;
        Vector3 _o;

        sealed class Deck
        {
            public bool playing;
            public float pos, jog, recBpm, pitch, playPress = -9f, padTime = -9f;
            public int pad;
            public bool loop;
        }
        readonly Deck[] _d = { new Deck(), new Deck() };
        readonly float[] _fader = new float[2], _low = new float[2], _filter = new float[2], _meter = new float[2];
        float _t, _bpm;
        int _shot = -1;
        bool _calmWas;

        Vector3 CT(float x, float z, float y = 0f) => _o + new Vector3((x - 320f) * 0.01f, y * 0.01f, (z - 180f) * 0.01f);

        protected override void Build()
        {
            _m = new GearMats(gearMat);
            _booth = new Booth(transform, clubMat, _m);
            _plate = _m.Tex(plateTex, new Color(0.025f, 0.025f, 0.028f), 0.2f, 0.4f);
            _ui = _m.M(0f, 0f, 0f, 0f, 0.5f, 0f, 10f); _ui.SetTexture("_MainTex", uiTex);
            _jogTop = _m.M(0.35f, 0.36f, 0.38f, 1f, 0.3f, 0.75f);
            _jogRing = _m.M(0.02f, 0.02f, 0.022f, 0f, 0.15f);
            _jogDisp = _m.M(0f, 0f, 0f, 0f, 0.5f, 0f, 6f);
            _alu = _m.M(0.55f, 0.56f, 0.6f, 1f, 0.35f, 0.4f);
            _keys = _m.M(0.02f, 0.02f, 0.022f, 0f, 0.5f);

            _body = GearMesh.RoundedBox(new Vector3(6.4f, 0.55f, 3.6f), 0.08f, 3, "controller");
            _foot = GearMesh.Lathe(new[] { new Vector2(0f, 0f), new Vector2(0.2f, 0f), new Vector2(0.2f, 0f), new Vector2(0.2f, -0.07f), new Vector2(0f, -0.07f) }, 24, "foot");
            _jogTopM = GearMesh.Lathe(new[] { new Vector2(0f, 0.12f), new Vector2(0.64f, 0.12f), new Vector2(0.66f, 0.11f), new Vector2(0.66f, 0.11f), new Vector2(0.66f, 0.06f) }, 96, "ctl jog top");
            _jogRingM = GearMesh.Lathe(new[] { new Vector2(0.67f, 0.10f), new Vector2(0.76f, 0.09f), new Vector2(0.78f, 0.06f), new Vector2(0.78f, 0.06f), new Vector2(0.78f, 0f) }, 96, "ctl jog ring");
            _jogGlow = GearMesh.Lathe(new[] { new Vector2(0.662f, 0.105f), new Vector2(0.67f, 0.095f) }, 96, "ctl jog glow");
            _jogDispM = GearMesh.Lathe(new[] { new Vector2(0f, 0.125f), new Vector2(0.22f, 0.125f), new Vector2(0.22f, 0.125f), new Vector2(0.22f, 0.12f) }, 48, "ctl jog display");
            _pad = GearMesh.RoundedBox(new Vector3(0.33f, 0.06f, 0.29f), 0.035f, 3, "ctl pad");
            _bigBtn = GearMesh.Lathe(new[] { new Vector2(0f, 0.06f), new Vector2(0.11f, 0.055f), new Vector2(0.12f, 0.045f), new Vector2(0.12f, 0.045f), new Vector2(0.12f, 0f) }, 40, "ctl big");
            _bigRing = GearMesh.Lathe(new[] { new Vector2(0.125f, 0.035f), new Vector2(0.145f, 0.03f), new Vector2(0.145f, 0.03f), new Vector2(0.145f, 0f) }, 40, "ctl ring");
            _small = GearMesh.RoundedBox(new Vector3(0.20f, 0.045f, 0.10f), 0.02f, 2, "ctl button");
            _knobS = GearMesh.Knob(0.065f, 0.13f, "ctl knob");
            _knobM = GearMesh.Knob(0.085f, 0.15f, "ctl knob m");
            _cap = GearMesh.RoundedBox(new Vector3(0.16f, 0.12f, 0.22f), 0.03f, 3, "ctl fader");
            _capLine = GearMesh.RoundedBox(new Vector3(0.14f, 0.004f, 0.014f), 0.001f, 1, "ctl fader line");
            _xfCap = GearMesh.RoundedBox(new Vector3(0.22f, 0.12f, 0.16f), 0.03f, 3, "ctl xf");
            _meterQ = GearMesh.Quad(0.032f, 0.80f, "ctl meter");
            // Laptop on a stand behind.
            _stand = GearMesh.RoundedBox(new Vector3(0.12f, 1.6f, 1.6f), 0.04f, 2, "stand leg");
            _lapBase = GearMesh.RoundedBox(new Vector3(3.3f, 0.16f, 2.3f), 0.08f, 3, "laptop base");
            _lapKeys = GearMesh.RoundedBox(new Vector3(2.8f, 0.02f, 1.0f), 0.01f, 1, "keyboard");
            _lapPad = GearMesh.RoundedBox(new Vector3(1.1f, 0.01f, 0.65f), 0.03f, 1, "trackpad");
            _lid = GearMesh.RoundedBox(new Vector3(3.3f, 2.2f, 0.06f), 0.06f, 3, "lid");
            _screen = GearMesh.Quad(3.1f, 1.94f, "laptop screen");
            _o = new Vector3(0f, TopY, -0.6f);
            _d[0].recBpm = 124f; _d[1].recBpm = 126f;
            _d[0].playing = true; _d[0].pos = 96f;
        }

        static Matrix4x4 T(Vector3 p) => GearDraw.T(p);
        static Matrix4x4 TRS(Vector3 p, Quaternion q) => GearDraw.TRS(p, q);

        void Knob(Mesh mesh, float r, float x, float z, float val, bool centred)
        {
            float a = centred ? val * 135f : -135f + 270f * Mathf.Clamp01(val);
            var pr = GearDraw.Props; pr.SetFloat("_Spin", r);
            GearDraw.Draw(mesh, _m.knob, TRS(CT(x, z), Quaternion.Euler(0f, a, 0f)), pr);
        }

        void Mix(float dt)
        {
            var mc = MixClock.At(rx);
            var P = _d[mc.p]; var N = _d[mc.n];
            float w = mc.w;
            bool nPlay = w >= 24f;
            if (nPlay != N.playing) { N.playing = nPlay; N.playPress = _t; }
            if (!P.playing && w < 24f) { P.playing = true; P.playPress = _t; }
            if (!N.playing && w >= 10f && w < 11f && N.pos != 0f) { N.pos = 0f; N.jog = 0f; }
            bool calm = rx.calm > 0.5f;
            if (calm && !_calmWas) P.loop = true;
            if (rx.dropped) { P.loop = false; P.pad = Mathf.FloorToInt(Mathf.Repeat(mc.cyc * 3f + rx.beatS * 0.25f, 4f)); P.padTime = _t; }
            _calmWas = calm;
            N.loop = false;
            foreach (var d in _d)
            {
                d.pitch = Mathf.Clamp(_bpm / d.recBpm - 1f, -0.08f, 0.08f);
                if (!d.playing) continue;
                float beats = dt * _bpm / 60f;
                if (d.loop) { float l0 = Mathf.Floor(d.pos / 4f) * 4f; d.pos += beats; if (d.pos >= l0 + 4f) d.pos -= 4f; }
                else d.pos += beats;
                d.jog += dt * 33.333f / 60f * (1f + d.pitch);
            }
            // The two-channel mixer.
            int chP = mc.p, chN = mc.n;
            _fader[chP] = 1f - MixClock.Ease((w - 28f) / 4f);
            _fader[chN] = MixClock.Ease((w - 24f) / 4f);
            _low[chN] = w < 28f ? -1f : Mathf.Lerp(-1f, 0f, MixClock.Ease((w - 28f) * 2f));
            _low[chP] = w < 28f ? 0f : Mathf.Lerp(0f, -1f, MixClock.Ease((w - 28f) * 2f));
            _filter[chP] = Mathf.Lerp(_filter[chP], rx.tension * 0.8f, 1f - Mathf.Exp(-dt * (rx.tension > _filter[chP] ? 2f : 10f)));
            _filter[chN] = Mathf.Lerp(_filter[chN], 0f, 1f - Mathf.Exp(-dt * 4f));
            float body = Mathf.Clamp01(0.32f + 0.5f * rx.bassFast + 0.35f * rx.midFast + 0.15f * rx.highFast);
            for (int c = 0; c < 2; c++)
                _meter[c] = Eased.Follow(_meter[c], body * _fader[c] * (1f - 0.35f * Mathf.Clamp01(-_low[c])), 20f, 4f, dt);
        }

        void DrawDeck(int side)
        {
            var d = _d[side];
            float jx = side == 0 ? 130f : 510f, tx = side == 0 ? 230f : 615f, px0 = side == 0 ? 65f : 455f, playx = side == 0 ? 25f : 410f;
            var jog = CT(jx, 200f);
            GearDraw.Draw(_jogRingM, _jogRing, T(jog));
            GearDraw.Draw(_jogTopM, _jogTop, T(jog));
            {
                var pr = GearDraw.Props; pr.SetColor("_Emit", (side == 0 ? new Color(0.3f, 0.7f, 3f) : new Color(3f, 1.2f, 0.2f)) * 0.8f * leds);
                GearDraw.Draw(_jogGlow, _m.led, T(jog), pr);
                pr = GearDraw.Props;
                pr.SetFloat("_Smear", d.jog); pr.SetFloat("_Level", Mathf.Clamp01(d.pos / 1024f)); pr.SetColor("_Emit", Color.white * glow);
                GearDraw.Draw(_jogDispM, _jogDisp, T(jog), pr);
            }
            // Pads: hot cues (one fires on the drop), a roll stepping up the top row through a
            // build, a beat loop held on the bottom row through a breakdown.
            bool roll = rx.tension > 0.3f && d.playing && !d.loop;
            int rollPad = Mathf.Clamp(Mathf.FloorToInt((rx.tension - 0.3f) / 0.7f * 4f), 0, 3);
            for (int r = 0; r < 2; r++)
                for (int c = 0; c < 4; c++)
                {
                    int i = r * 4 + c;
                    Color col; float on;
                    if (d.loop) { col = new Color(1f, 0.45f, 0.05f); on = r == 1 && c == 1 ? 1.4f : 0.12f; }
                    else if (roll) { col = new Color(0.2f, 0.5f, 1f); on = r == 0 && c == rollPad ? 1.5f : 0.15f; }
                    else { col = PadCol[i]; on = 0.35f + (i == d.pad ? 3f * Mathf.Clamp01(1f - (_t - d.padTime) / 0.6f) : 0f); }
                    var pr = GearDraw.Props; pr.SetColor("_Emit", col * on * leds); pr.SetColor("_Tint", col * 0.05f);
                    GearDraw.Draw(_pad, _m.led, T(CT(px0 + c * 40f, 75f - r * 35f, 1f)), pr);
                }
            // PLAY / CUE / SYNC.
            float playLed = d.playing ? 1f : (Mathf.Repeat(_t * 1.6f, 1f) < 0.5f ? 0.8f : 0.08f);
            float press = Mathf.Clamp01(1f - (_t - d.playPress) / 0.18f);
            {
                var pr = GearDraw.Props; pr.SetColor("_Emit", new Color(0.2f, 3f, 0.8f) * playLed * leds);
                GearDraw.Draw(_bigRing, _m.led, T(CT(playx, 30f)), pr);
                GearDraw.Draw(_bigBtn, _keys, T(CT(playx, 30f, -2f * press)));
                pr = GearDraw.Props; pr.SetColor("_Emit", new Color(3f, 1.2f, 0.1f) * (d.playing ? 0.15f : 1f) * leds);
                GearDraw.Draw(_bigRing, _m.led, T(CT(playx, 68f)), pr);
                GearDraw.Draw(_bigBtn, _keys, T(CT(playx, 68f)));
                pr = GearDraw.Props; pr.SetColor("_Emit", new Color(0.3f, 0.8f, 3f) * (d.playing ? 0.9f : 0.08f) * leds);
                GearDraw.Draw(_small, _m.led, T(CT(playx, 103f, 1f)), pr);
            }
            // Tempo slider.
            float tz = 200f - d.pitch * 1000f;
            GearDraw.Draw(_cap, _m.cap, T(CT(tx, tz, 5f)));
            GearDraw.Draw(_capLine, _m.line, T(CT(tx, tz, 11.1f)));
            // Loop, beat FX.
            for (int i = 0; i < 3; i++)
            {
                var pr = GearDraw.Props; pr.SetColor("_Emit", new Color(3f, 1.3f, 0.1f) * (d.loop && i < 2 ? 0.9f : 0.05f) * leds);
                GearDraw.Draw(_small, _m.led, T(CT(jx - 60f + i * 30f, 312f, 1f)), pr);
            }
            Knob(_knobS, 0.08f, jx + 50f, 330f, 0.4f, false);
            Knob(_knobS, 0.07f, jx + 80f, 330f, 0f, true);
        }

        void DrawMixer()
        {
            for (int ch = 0; ch < 2; ch++)
            {
                float cx = ch == 0 ? 285f : 355f;
                Knob(_knobS, 0.07f, cx, 318f, 0.5f, false);
                Knob(_knobS, 0.07f, cx, 288f, 0f, true);
                Knob(_knobS, 0.07f, cx, 260f, 0f, true);
                Knob(_knobS, 0.07f, cx, 232f, _low[ch], true);
                Knob(_knobM, 0.09f, cx, 198f, _filter[ch], true);
                float fz = 55f + 90f * _fader[ch];
                GearDraw.Draw(_cap, _m.cap, T(CT(cx, fz, 5f)));
                GearDraw.Draw(_capLine, _m.line, T(CT(cx, fz, 11.1f)));
                var pr = GearDraw.Props; pr.SetFloat("_Level", _meter[ch]);
                GearDraw.Draw(_meterQ, _m.meter, T(CT(ch == 0 ? 314f : 326f, 260f, 0.2f)), pr);
                pr = GearDraw.Props; pr.SetColor("_Emit", new Color(3f, 1.2f, 0.1f) * (_cue[ch] ? 1f : 0.05f) * leds);
                GearDraw.Draw(_small, _m.led, T(CT(cx, 168f, 1f)), pr);
            }
            Knob(_knobM, 0.10f, 320f, 336f, 0.3f, false);
            Knob(_knobS, 0.07f, 320f, 190f, 0.7f, false);
            Knob(_knobS, 0.06f, 320f, 160f, -0.3f, true);
            GearDraw.Draw(_small, _m.cap, T(CT(298f, 335.5f, 1f)));
            GearDraw.Draw(_small, _m.cap, T(CT(342f, 335.5f, 1f)));
            GearDraw.Draw(_xfCap, _m.cap, T(CT(320f, 28.5f, 5f)));
            GearDraw.Draw(_capLine, _m.line, TRS(CT(320f, 28.5f, 11.1f), Quaternion.Euler(0f, 90f, 0f)));
        }

        readonly bool[] _cue = new bool[2];

        void DrawLaptop()
        {
            var b = _o + new Vector3(0f, 1.0f, 3.3f);
            foreach (var x in new[] { -1.3f, 1.3f }) GearDraw.Draw(_stand, _m.cap, T(new Vector3(x, 0.8f, b.z)));
            GearDraw.Draw(_lapBase, _alu, T(b + new Vector3(0f, 0.08f, 0f)));
            GearDraw.Draw(_lapKeys, _keys, T(b + new Vector3(0f, 0.17f, 0.35f)));
            GearDraw.Draw(_lapPad, _keys, T(b + new Vector3(0f, 0.165f, -0.6f)));
            var hinge = b + new Vector3(0f, 0.16f, 1.15f);
            var lidM = TRS(hinge, Quaternion.Euler(12f, 0f, 0f)) * T(new Vector3(0f, 1.1f, 0f));
            GearDraw.Draw(_lid, _alu, lidM);
            var pr = GearDraw.Props;
            pr.SetFloat("_Spin", _d[0].pos); pr.SetFloat("_Smear", _d[1].pos);
            pr.SetColor("_Emit", Color.white * 1.1f * glow);
            GearDraw.Draw(_screen, _ui, lidM * TRS(new Vector3(0f, 0f, -0.035f), Quaternion.Euler(-90f, 0f, 0f)), pr);
        }

        protected override void Frame(ShowState s, float dt)
        {
            dt = Mathf.Clamp(dt, 1e-4f, 1f / 20f);
            _t += dt;
            _bpm = Mathf.Clamp(s.bpm, 60f, 200f);
            GearDraw.BeginFrame();
            Mix(dt);
            var mc = MixClock.At(rx);
            _cue[mc.n] = mc.w >= 20f && mc.w < 28f; _cue[mc.p] = false;
            _booth.Frame(rx, lamp, club);
            GearDraw.Draw(_body, _plate, T(CT(320f, 180f, -27.5f)));
            foreach (var f in new[] { new Vector2(40f, 40f), new Vector2(600f, 40f), new Vector2(40f, 320f), new Vector2(600f, 320f) })
                GearDraw.Draw(_foot, _m.rubber, T(CT(f.x, f.y, -55f)));
            DrawDeck(0); DrawDeck(1);
            DrawMixer();
            DrawLaptop();

            // Shots, following the mix.
            float w = mc.w;
            int kind; float s0, s1; int deck = mc.p;
            if (w < 8f) { kind = 0; s0 = 0f; s1 = 8f; }
            else if (w < 12f) { kind = 1; s0 = 8f; s1 = 12f; }
            else if (w < 16f) { kind = 4; s0 = 12f; s1 = 16f; }
            else if (w < 20f) { kind = 2; s0 = 16f; s1 = 20f; }
            else if (w < 24f) { kind = 3; deck = mc.n; s0 = 20f; s1 = 24f; }
            else if (w < 28f) { kind = 1; deck = mc.n; s0 = 24f; s1 = 28f; }
            else { kind = 3; s0 = 28f; s1 = 32f; }
            int id = mc.cyc * 64 + Mathf.FloorToInt(s0);
            if (id != _shot) { _shot = id; rig.Snap(); }
            float u = Mathf.Clamp01((w - s0) / (s1 - s0));
            float sw = Mathf.Sin(u * Mathf.PI - Mathf.PI * 0.5f) * drift;
            float jx = deck == 0 ? 130f : 510f, px = deck == 0 ? 125f : 515f;
            Vector3 pos, look; float fstop = 5.6f;
            switch (kind)
            {
                default:
                case 0: look = CT(320f, 260f, 60f); pos = look + new Vector3(sw * 1.5f, 3.6f, -7.5f); fstop = 8f; break;
                case 1: look = CT(jx, 190f); pos = look + new Vector3(-1.0f - 0.3f * sw, 1.1f, -1.9f); break;
                case 2: look = CT(px, 60f); pos = look + new Vector3(0.6f * sw, 1.2f, -1.6f); break;
                case 3: look = CT(320f, 180f); pos = look + new Vector3(0.7f - 0.5f * sw, 2.0f, -2.2f); break;
                case 4: look = _o + new Vector3(0f, 2.24f, 4.6f); pos = look + new Vector3(0.6f * sw, 0.3f, -3.4f); break;
            }
            rig.Move(cam, pos, look, dt, 3f);
            HqPost.Focus(Vector3.Distance(cam.transform.position, look), fstop, 50f);
        }
    }
}
