// unity_cdj: close-ups of a club booth with two media players either side of
// the four-channel mixer, modelled on the real layout (tools/gear_textures.py
// draws the faceplate): a raised, angled screen with the waveform, the source
// buttons and browse knob beside it, eight hot cue pads, LOOP IN/OUT, a 206 mm
// jog with a dimpled ring, a lit edge and a round display in its centre, CUE
// and PLAY/PAUSE bottom-left, and the tempo slider, MASTER TEMPO, BEAT SYNC and
// MASTER on the right. The same 32-bar mix as unity_decks, told on players:
//  - bar 10 the idle player loads the next track; bar 20 it is cued (CUE
//    lit, PLAY blinking); bar 24 PLAY; the mixer crosses 24..32 with a bass
//    swap; bar 32 the outgoing player pauses.
//  - Shape: the meters, the waveform's peaks and the filter knob follow the
//    music; a hot cue pad fires on the drop; a breakdown sets a 4-beat loop
//    (IN/OUT blink, the loop shaded on the screen) that releases on the drop.
//  - Motion: the waveforms scroll at the tempo, the jog display's playhead
//    turns like a record (33 1/3 rpm times the pitch), the tempo slider sits
//    where the tempo needs it. The camera cuts on bar lines, following the mix.
// Controls: drift, booth lamp, club lights, screen glow, LED glow.
using UnityEngine;

namespace TrippinStage
{
    public sealed class CdjShow : KitShow
    {
        public Material gearMat, clubMat;
        public Texture2D plateTex, mixTex, fxTex, screen0, screen1;

        [TrippinParam("Drift", 0f, 2f)] public float drift = 1f;
        [TrippinParam("Booth lamp", 0.3f, 2f)] public float lamp = 1f;
        [TrippinParam("Club lights", 0f, 2f)] public float club = 1f;
        [TrippinParam("Screen glow", 0.3f, 2f)] public float glow = 1f;
        [TrippinParam("LED glow", 0.3f, 2f)] public float leds = 1f;

        const float TopY = 1.10f, MixY = 1.08f, PlayerX = 3.315f, PlayerZ = -0.04f;
        static readonly Color[] PadCol =
        {
            new Color(0.1f, 1f, 0.2f), new Color(0.1f, 1f, 0.2f), new Color(1f, 0.5f, 0.05f), new Color(0.1f, 0.4f, 1f),
            new Color(1f, 0.1f, 0.4f), new Color(0.1f, 1f, 0.2f), new Color(0.6f, 0.1f, 1f), new Color(1f, 0.85f, 0.1f),
        };

        GearMats _m;
        ClubMixer _mixer;
        Booth _booth;
        Material _plate, _screenM, _jogDisp, _jogRing, _jogTop, _pad;
        Mesh _body, _foot, _slot, _pod, _screenQ, _jogTopM, _jogRingM, _jogDispM, _jogGlow, _bigBtn, _bigRing, _smallBtn,
             _padM, _roundBtn, _tempoCap, _tempoLine, _knobS, _browse, _browseRing, _lever;
        Texture2D[] _screens;

        sealed class Player
        {
            public Vector3 o;                 // top-plate centre (world)
            public bool playing;
            public float pos;                 // beats into the track
            public float playPress = -9f, cuePress = -9f, padTime = -9f;
            public int pad, track;
            public float recBpm, pitch, jog;  // jog: the display's playhead, revolutions
            public bool loop;
            public Vector3 CJ(float x, float z, float y = 0f) => o + new Vector3((x - 160f) * 0.01f, y * 0.01f, (z - 203f) * 0.01f);
            public Vector3 Jog => CJ(165f, 160f);
        }
        readonly Player[] _p = { new Player(), new Player() };
        float _t, _bpm;
        int _shot = -1;
        bool _calmWas;

        public override bool UsesCamera => true;

        protected override void Build()
        {
            _m = new GearMats(gearMat);
            _booth = new Booth(transform, clubMat, _m);
            _mixer = new ClubMixer(_m, mixTex, fxTex, new Vector3(0f, MixY, 0f));
            _screens = new[] { screen0, screen1 };
            _plate = _m.Tex(plateTex, new Color(0.02f, 0.02f, 0.022f), 0.2f, 0.3f);
            _screenM = _m.M(0f, 0f, 0f, 0f, 0.5f, 0f, 7f);
            _jogDisp = _m.M(0f, 0f, 0f, 0f, 0.5f, 0f, 6f);
            _jogRing = _m.M(0.02f, 0.02f, 0.022f, 0f, 0.2f, 0f, 8f);
            _jogTop = _m.M(0.09f, 0.09f, 0.1f, 1f, 0.28f, 0.75f);
            _pad = _m.M(0.03f, 0.03f, 0.032f, 0f, 0.6f);

            _body = GearMesh.RoundedBox(new Vector3(3.20f, 1.0f, 4.06f), 0.04f, 3, "player");
            _foot = GearMesh.Lathe(new[] { new Vector2(0f, 0f), new Vector2(0.25f, 0f), new Vector2(0.25f, 0f), new Vector2(0.25f, -0.12f), new Vector2(0f, -0.12f) }, 32, "player foot");
            _slot = GearMesh.RoundedBox(new Vector3(1.6f, 0.07f, 0.03f), 0.012f, 2, "disc slot");
            _pod = GearMesh.RoundedBox(new Vector3(2.2f, 0.30f, 1.10f), 0.05f, 3, "display pod");
            _screenQ = GearMesh.Quad(1.56f, 0.88f, "player screen");
            _jogTopM = GearMesh.Lathe(new[] { new Vector2(0f, 0.19f), new Vector2(0.78f, 0.19f), new Vector2(0.80f, 0.18f), new Vector2(0.80f, 0.18f), new Vector2(0.80f, 0.10f) }, 96, "jog top");
            _jogRingM = GearMesh.Lathe(new[] { new Vector2(0.81f, 0.16f), new Vector2(0.95f, 0.15f), new Vector2(1.01f, 0.12f), new Vector2(1.03f, 0.08f), new Vector2(1.03f, 0.08f), new Vector2(1.03f, 0f) }, 120, "jog ring");
            _jogDispM = GearMesh.Lathe(new[] { new Vector2(0f, 0.195f), new Vector2(0.36f, 0.195f), new Vector2(0.36f, 0.195f), new Vector2(0.36f, 0.19f) }, 64, "jog display");
            _jogGlow = GearMesh.Lathe(new[] { new Vector2(0.802f, 0.175f), new Vector2(0.81f, 0.165f) }, 96, "jog glow");
            _bigBtn = GearMesh.Lathe(new[] { new Vector2(0f, 0.08f), new Vector2(0.15f, 0.075f), new Vector2(0.17f, 0.06f), new Vector2(0.17f, 0.06f), new Vector2(0.17f, 0f) }, 48, "big button");
            _bigRing = GearMesh.Lathe(new[] { new Vector2(0.175f, 0.045f), new Vector2(0.205f, 0.04f), new Vector2(0.205f, 0.04f), new Vector2(0.205f, 0f) }, 48, "big ring");
            _smallBtn = GearMesh.RoundedBox(new Vector3(0.16f, 0.045f, 0.085f), 0.018f, 2, "small button");
            _padM = GearMesh.RoundedBox(new Vector3(0.21f, 0.06f, 0.12f), 0.025f, 2, "hot cue pad");
            _roundBtn = GearMesh.Lathe(new[] { new Vector2(0f, 0.05f), new Vector2(0.08f, 0.045f), new Vector2(0.085f, 0.035f), new Vector2(0.085f, 0.035f), new Vector2(0.085f, 0f) }, 32, "round button");
            _tempoCap = GearMesh.RoundedBox(new Vector3(0.24f, 0.14f, 0.20f), 0.03f, 3, "tempo cap");
            _tempoLine = GearMesh.RoundedBox(new Vector3(0.21f, 0.004f, 0.016f), 0.001f, 1, "tempo line");
            _knobS = GearMesh.Knob(0.065f, 0.13f, "jog adjust");
            _browse = GearMesh.Knob(0.13f, 0.20f, "browse");
            _browseRing = GearMesh.Lathe(new[] { new Vector2(0.135f, 0.05f), new Vector2(0.155f, 0.045f), new Vector2(0.155f, 0.045f), new Vector2(0.155f, 0f) }, 48, "browse ring");
            _lever = GearMesh.RoundedBox(new Vector3(0.10f, 0.12f, 0.22f), 0.03f, 2, "direction lever");

            _p[0].o = new Vector3(-PlayerX, TopY, PlayerZ); _p[1].o = new Vector3(PlayerX, TopY, PlayerZ);
            _p[0].recBpm = 124f; _p[1].recBpm = 126f;
            _p[0].track = 0; _p[1].track = 1;
            _p[0].playing = true; _p[0].pos = 96f;
        }

        static Matrix4x4 T(Vector3 p) => GearDraw.T(p);
        static Matrix4x4 TRS(Vector3 p, Quaternion q) => GearDraw.TRS(p, q);

        void Mix(float dt)
        {
            var mc = MixClock.At(rx);
            var P = _p[mc.p]; var N = _p[mc.n];
            _mixer.Update(rx, mc, dt, _bpm);
            float w = mc.w;
            // PLAY on the incoming player at 24; the outgoing one pauses at 32 (bar 0).
            bool nPlay = w >= 24f;
            if (nPlay != N.playing) { N.playing = nPlay; N.playPress = _t; }
            if (!P.playing && w < 24f) { P.playing = true; P.playPress = _t; }
            // The idle player loads the next track at 10 and is cued at 20 (back to the cue point).
            if (!N.playing && w >= 10f && w < 11f && N.pos != 0f) { N.track = (mc.cyc + mc.n + 1) % 2; N.pos = 0f; N.jog = 0f; }
            if (!N.playing && w >= 20f && N.cuePress < _t - 30f) N.cuePress = _t;
            // A breakdown sets a 4-beat loop on the playing player; the drop fires a hot cue and releases it.
            bool calm = rx.calm > 0.5f;
            if (calm && !_calmWas) P.loop = true;
            if (rx.dropped) { P.loop = false; P.pad = Mathf.FloorToInt(Mathf.Repeat(mc.cyc * 3f + rx.beatS * 0.25f, 8f)); P.padTime = _t; }
            _calmWas = calm;
            N.loop = false;
            foreach (var p in _p)
            {
                p.pitch = Mathf.Clamp(_bpm / p.recBpm - 1f, -0.10f, 0.10f);
                if (!p.playing) continue;
                float beats = dt * _bpm / 60f;
                if (p.loop)
                {
                    float l0 = Mathf.Floor(p.pos / 4f) * 4f;
                    p.pos += beats;
                    if (p.pos >= l0 + 4f) p.pos -= 4f;
                }
                else p.pos += beats;
                p.jog += dt * 33.333f / 60f * (1f + p.pitch);
            }
        }

        void DrawPlayer(Player p, int idx)
        {
            var knob = _m.knob; var cap = _m.cap; var led = _m.led;
            GearDraw.Draw(_body, _plate, T(p.CJ(160f, 203f, -50f)));
            foreach (var f in new[] { new Vector2(40f, 40f), new Vector2(280f, 40f), new Vector2(40f, 366f), new Vector2(280f, 366f) })
                GearDraw.Draw(_foot, _m.rubber, T(p.CJ(f.x, f.y, -100f)));
            GearDraw.Draw(_slot, _m.gloss, T(p.CJ(160f, -1f, -55f)));
            // The display pod, tipped toward the DJ, and its screen.
            var podQ = Quaternion.Euler(-14f, 0f, 0f);
            var podM = TRS(p.CJ(170f, 352f, 12f), podQ);
            GearDraw.Draw(_pod, _m.gloss, podM);
            {
                var pr = GearDraw.Props;
                pr.SetTexture("_MainTex", _screens[p.track]);
                pr.SetFloat("_Spin", p.pos);
                pr.SetFloat("_Level", Mathf.Clamp01(p.pos / 1024f));
                pr.SetFloat("_Smear", p.loop ? 1f : 0f);
                pr.SetColor("_Emit", Color.white * 1.15f * glow);
                GearDraw.Draw(_screenQ, _screenM, podM * T(new Vector3(0f, 0.152f, -0.08f)), pr);
            }
            for (int i = 0; i < 4; i++) GearDraw.Draw(_smallBtn, cap, podM * T(new Vector3(-0.72f + i * 0.48f, 0.152f, 0.45f)));
            // Source buttons, browse knob, BACK, TAG TRACK.
            for (int i = 0; i < 4; i++) GearDraw.Draw(_smallBtn, cap, T(p.CJ(33f, 392f - i * 22f, 1f)));
            GearDraw.Draw(_browseRing, _m.alu, T(p.CJ(300f, 372f)));
            {
                var pr = GearDraw.Props; pr.SetFloat("_Spin", 0.13f);
                GearDraw.Draw(_browse, knob, TRS(p.CJ(300f, 372f), Quaternion.Euler(0f, 30f + idx * 70f, 0f)), pr);
            }
            GearDraw.Draw(_smallBtn, cap, T(p.CJ(300f, 341.5f, 1f)));
            GearDraw.Draw(_smallBtn, cap, T(p.CJ(300f, 319.5f, 1f)));
            // Hot cue pads: stored cues glow dim in their colours; a fired one flashes.
            for (int i = 0; i < 8; i++)
            {
                var pr = GearDraw.Props;
                float fire = i == p.pad ? Mathf.Clamp01(1f - (_t - p.padTime) / 0.6f) : 0f;
                pr.SetColor("_Emit", PadCol[i] * (0.35f + 3.5f * fire) * leds);
                pr.SetColor("_Tint", PadCol[i] * 0.06f);
                GearDraw.Draw(_padM, led, T(p.CJ(78f + i * 26f, 283.5f, 1f)), pr);
            }
            // Loop: IN/CUE and OUT blink on the beat while a loop runs; RELOOP/EXIT.
            float blink = p.loop && Mathf.Repeat(rx.beatS, 1f) < 0.5f ? 1f : 0.08f;
            foreach (var x in new[] { 24f, 48f })
            {
                var pr = GearDraw.Props; pr.SetColor("_Emit", new Color(3f, 1.3f, 0.1f) * blink * leds);
                pr.SetColor("_Tint", new Color(0.2f, 0.1f, 0.02f));
                GearDraw.Draw(_roundBtn, led, T(p.CJ(x, 262f)), pr);
            }
            {
                var pr = GearDraw.Props; pr.SetColor("_Emit", new Color(3f, 1.3f, 0.1f) * (p.loop ? 0.8f : 0.05f) * leds);
                GearDraw.Draw(_smallBtn, led, T(p.CJ(36f, 236f, 1f)), pr);
            }
            // Left column.
            foreach (var z in new[] { 128f, 158f })
                foreach (var x in new[] { 22f, 47f }) GearDraw.Draw(_smallBtn, cap, T(p.CJ(x, z, 1f)));
            GearDraw.Draw(_lever, cap, TRS(p.CJ(34f, 201f, 4f), Quaternion.Euler(-20f, 0f, 0f)));
            // CUE (orange ring) and PLAY/PAUSE (green ring): PLAY blinks while paused,
            // CUE lights when cued; each sinks a little when pressed.
            bool cued = !p.playing && _t - p.cuePress < 30f;
            float playLed = p.playing ? 1f : (Mathf.Repeat(_t * 1.6f, 1f) < 0.5f ? 0.8f : 0.08f);
            float cueLed = cued ? 1f : p.playing ? 0.1f : 0.25f;
            DrawBig(p.CJ(35f, 40f), new Color(0.2f, 3.2f, 0.9f) * playLed, _t - p.playPress);
            DrawBig(p.CJ(35f, 95f), new Color(3.2f, 1.2f, 0.1f) * cueLed, _t - p.cuePress);
            // Jog: dimpled ring, metal top, lit edge, the display.
            var jog = p.Jog;
            GearDraw.Draw(_jogRingM, _jogRing, T(jog));
            GearDraw.Draw(_jogTopM, _jogTop, T(jog));
            {
                var pr = GearDraw.Props; pr.SetColor("_Emit", new Color(0.25f, 0.6f, 3f) * (0.7f + 0.5f * rx.bassFast) * leds);
                GearDraw.Draw(_jogGlow, led, T(jog), pr);
            }
            {
                var pr = GearDraw.Props;
                pr.SetFloat("_Smear", p.jog);
                pr.SetFloat("_Level", Mathf.Clamp01(p.pos / 1024f));
                pr.SetColor("_Emit", Color.white * glow);
                GearDraw.Draw(_jogDispM, _jogDisp, T(jog), pr);
            }
            // Right column: tempo slider (+ is toward the DJ), MASTER TEMPO, range, BEAT SYNC, MASTER, JOG MODE.
            float tz = 106f - p.pitch * 100f * 6.8f;
            GearDraw.Draw(_tempoCap, cap, T(p.CJ(297.5f, tz, 6f)));
            GearDraw.Draw(_tempoLine, _m.line, T(p.CJ(297.5f, tz, 13.1f)));
            {
                var pr = GearDraw.Props; pr.SetColor("_Emit", new Color(3f, 0.4f, 0.1f) * 0.9f * leds);
                GearDraw.Draw(_smallBtn, led, T(p.CJ(297.5f, 192f, 1f)), pr);
            }
            GearDraw.Draw(_smallBtn, cap, T(p.CJ(297.5f, 210f, 1f)));
            {
                var pr = GearDraw.Props; pr.SetColor("_Emit", new Color(0.3f, 0.8f, 3f) * (p.playing ? 1f : 0.1f) * leds);
                GearDraw.Draw(_smallBtn, led, T(p.CJ(282f, 235f, 1f)), pr);
                pr = GearDraw.Props;
                bool master = p == _p[MixClock.At(rx).p];
                pr.SetColor("_Emit", new Color(3f, 1.4f, 0.1f) * (master ? 1f : 0.08f) * leds);
                GearDraw.Draw(_smallBtn, led, T(p.CJ(305f, 235f, 1f)), pr);
                pr = GearDraw.Props; pr.SetColor("_Emit", new Color(0.3f, 0.8f, 3f) * 0.9f * leds);
                GearDraw.Draw(_smallBtn, led, T(p.CJ(285f, 261f, 1f)), pr);
            }
            {
                var pr = GearDraw.Props; pr.SetFloat("_Spin", 0.065f);
                GearDraw.Draw(_knobS, knob, TRS(p.CJ(306f, 284f), Quaternion.Euler(0f, -40f, 0f)), pr);
            }
        }

        void DrawBig(Vector3 at, Color ring, float sincePress)
        {
            float press = Mathf.Clamp01(1f - sincePress / 0.18f);
            var pr = GearDraw.Props; pr.SetColor("_Emit", ring * leds);
            pr.SetColor("_Tint", ring.linear * 0.01f);
            GearDraw.Draw(_bigRing, _m.led, T(at), pr);
            GearDraw.Draw(_bigBtn, _m.chrome, T(at + Vector3.up * (-0.02f * press)));
        }

        void Shot(int kind, Player p, float u, out Vector3 pos, out Vector3 look, out float fstop)
        {
            float sw = Mathf.Sin(u * Mathf.PI - Mathf.PI * 0.5f) * drift;
            fstop = 5.6f;
            switch (kind)
            {
                default:
                case 0:   // the whole booth
                    look = new Vector3(0f, 1.0f, 0.3f);
                    pos = new Vector3(sw * 0.6f, 4.3f, -7.4f + 0.4f * u * drift);
                    fstop = 8f;
                    break;
                case 1:   // low over the jog to its display and the pads
                    look = p.Jog + new Vector3(0f, 0.2f, 0.25f);
                    pos = look + new Vector3(-1.1f - 0.3f * sw, 1.1f, -2.0f);
                    break;
                case 2:   // the screen: waveform scrolling past the playhead
                {
                    var sc = p.CJ(170f, 345f, 30f);
                    look = sc;
                    pos = sc + new Vector3(0.25f * sw, 1.35f, -1.75f);
                    break;
                }
                case 3:   // hot cue pads and the loop buttons
                    look = p.CJ(130f, 270f, 2f);
                    pos = look + new Vector3(0.4f * sw - 0.3f, 1.3f, -1.5f);
                    break;
                case 4:   // CUE and PLAY/PAUSE
                    look = p.CJ(45f, 70f);
                    pos = look + new Vector3(0.5f + 0.2f * sw, 1.0f, -1.3f);
                    break;
                case 5:   // tempo slider, sync and master
                    look = p.CJ(290f, 150f);
                    pos = look + new Vector3(-0.6f - 0.2f * sw, 1.2f, -1.6f);
                    break;
                case 6:   // over the mixer's channel strips
                    look = _mixer.MX(166f, 300f);
                    pos = look + new Vector3(-0.7f * sw - 0.2f, 2.3f, -2.5f);
                    break;
                case 7:   // channel faders and crossfader
                    look = _mixer.MX(166f, 100f);
                    pos = look + new Vector3(0.9f - 0.5f * sw, 1.9f, -2.3f);
                    break;
            }
        }

        protected override void Frame(ShowState s, float dt)
        {
            dt = Mathf.Clamp(dt, 1e-4f, 1f / 20f);
            _t += dt;
            _bpm = Mathf.Clamp(s.bpm, 60f, 200f);
            GearDraw.BeginFrame();
            Mix(dt);
            _booth.Frame(rx, lamp, club);
            DrawPlayer(_p[0], 0);
            DrawPlayer(_p[1], 1);
            _mixer.Draw(rx, leds);

            // The shot: cut on bar lines, following the mix.
            var mc = MixClock.At(rx);
            float w = mc.w;
            Player P = _p[mc.p], N = _p[mc.n];
            int kind; Player pl; float s0, s1;
            if (w < 8f) { kind = 0; pl = P; s0 = 0f; s1 = 8f; }
            else if (w < 12f) { kind = 1; pl = P; s0 = 8f; s1 = 12f; }
            else if (w < 16f) { kind = P.loop ? 3 : 2; pl = P; s0 = 12f; s1 = 16f; }
            else if (w < 20f) { kind = 6; pl = P; s0 = 16f; s1 = 20f; }
            else if (w < 22f) { kind = 2; pl = N; s0 = 20f; s1 = 22f; }
            else if (w < 24f) { kind = 5; pl = N; s0 = 22f; s1 = 24f; }
            else if (w < 26f) { kind = 4; pl = N; s0 = 24f; s1 = 26f; }
            else if (w < 28f) { kind = 1; pl = N; s0 = 26f; s1 = 28f; }
            else { kind = 7; pl = P; s0 = 28f; s1 = 32f; }
            int shotId = mc.cyc * 64 + Mathf.FloorToInt(s0 * 2f);
            if (shotId != _shot) { _shot = shotId; rig.Snap(); }
            float u = Mathf.Clamp01((w - s0) / (s1 - s0));
            Shot(kind, pl, u, out var pos, out var look, out float fstop);
            rig.Move(cam, pos, look, dt, 3f);
            HqPost.Focus(Vector3.Distance(cam.transform.position, look), fstop, 50f);
        }
    }
}
