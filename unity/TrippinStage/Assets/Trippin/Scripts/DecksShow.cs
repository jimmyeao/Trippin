// unity_decks: close-ups of a real DJ setup: two direct-drive turntables and a
// four-channel club mixer in a booth, built to the proportions and layout of
// the real thing (see GearKit.cs, HqGear.shader). Nothing deforms: the music
// plays through the gear the way a DJ uses it, as a 32-bar mix that repeats
// with the decks swapping roles:
//  - bar 20 the incoming deck is cued (its CUE lights), bar 22 needle drop
//    (cue lever up, the arm swings to the lead-in and lowers), bar 24 START
//    (the platter spins up), faders cross 24..32 with a bass swap at 28, and
//    bar 32 STOP brakes the outgoing deck.
//  - Shape: the channel and master LED meters follow the eased band levels
//    through each fader and EQ; the COLOR knob sweeps the filter up through a
//    build and snaps back on the drop; BEAT FX fires for a bar on the drop.
//  - Motion: the platters run at 33 1/3 rpm times the pitch, and the pitch
//    fader sits where the tempo needs it (each record has its own BPM); the
//    strobe dots freeze on the row that matches, drifting on the others; the
//    tonearm creeps inward as the record plays. The camera cuts on bar lines
//    between close-ups that follow the action, drifting slowly in each.
// Controls: drift, booth lamp, club lights, strobe glow, LED glow.
using UnityEngine;

namespace TrippinStage
{
    public sealed class DecksShow : KitShow
    {
        public Material gearMat, clubMat;
        public Texture2D ttTex, startTex, mixTex, fxTex, slipTex, label0, label1, label2, label3;

        [TrippinParam("Drift", 0f, 2f)] public float drift = 1f;
        [TrippinParam("Booth lamp", 0.3f, 2f)] public float lamp = 1f;
        [TrippinParam("Club lights", 0f, 2f)] public float club = 1f;
        [TrippinParam("Strobe glow", 0f, 2f)] public float strobe = 1f;
        [TrippinParam("LED glow", 0.3f, 2f)] public float leds = 1f;

        const float TableY = 0f, DeckY = 1.27f, MixY = 1.08f, DeckX = 4.03f, DeckZ = -0.305f;
        const float ArmH = 0.40f, ArmLen = 2.30f;
        // At rest the arm points almost straight to the front (headshell inside the plate
        // by the front-right corner), lying in the rest just in front of its base.
        const float RestPhi = 81f;
        static readonly float[] RowPitch = { 0.066f, 0.033f, 0f, -0.033f };
        static readonly float[] RowDots = { 180f, 216f, 133f, 160f };

        // Materials.
        Material _plateTT, _rubber, _alu, _aluDark, _chrome, _knob, _knobLight, _startBtn, _vinyl, _slip, _strobeM,
                 _led, _meter, _screen, _plateMix, _cap, _line, _cart, _guard, _table, _club;
        // Meshes.
        Mesh _ttPlate, _ttBody, _foot, _platter, _band, _slipM, _record, _spindle, _powerBody, _powerCap, _startM, _speedM,
             _ledSmall, _target, _targetTip, _pitchCap, _pitchLine, _ringM, _baseAlu, _post, _gimbal, _armTube, _weight,
             _headshell, _cartM, _guardM, _lift, _collar, _restPost, _restCradle, _cueLever, _adapter, _hinge,
             _mixBody, _knobS, _knobM, _knobL, _cueBtn, _faderCap, _xfCap, _faderLine, _meterQ, _screenQ, _fxBtn, _onOff,
             _toggle, _tableM;
        Texture2D[] _labels;
        Vector3 _armC, _hd;      // headshell connector and direction, in arm space

        sealed class Deck
        {
            public Vector3 o;            // plate-top centre (world)
            public float ang, speed;     // platter angle (deg) and speed (rev/s)
            public bool on;
            public float press = -9f;    // time START/STOP was last pressed
            public float phi = RestPhi, lift, groove = 1.44f;   // arm angle from the spindle line (deg), lift 0..1
            public float recBpm = 124f, pitch;
            public int label;
            public readonly float[] rows = new float[4];
            public float playBars;       // bars since the needle went down
            public Vector3 TT(float x, float z, float y = 0f) => o + new Vector3((x - 226.5f) * 0.01f, y * 0.01f, (z - 176.5f) * 0.01f);
            public Vector3 Centre => TT(176f, 180f);
            public Vector3 Pivot => TT(375f, 265f);
        }
        readonly Deck[] _d = { new Deck(), new Deck() };
        Vector3 _mix;
        Vector3 MX(float x, float z, float y = 0f) => _mix + new Vector3((x - 166.5f) * 0.01f, y * 0.01f, (z - 207f) * 0.01f);

        // Mixer state.
        readonly float[] _fader = new float[4], _low = new float[4], _color = new float[4], _meterV = new float[4];
        readonly bool[] _cue = new bool[4];
        float _xf = 0.5f, _masterL, _masterR, _fxOn, _impS, _lastCut = -1f;
        int _shot = -1;
        float _shotStart;
        readonly Vector4[] _lp = new Vector4[4], _lc = new Vector4[4];
        float _t;

        public override bool UsesCamera => true;

        Material M(float r, float g, float b, float metal, float rough, float aniso = 0f, float mode = 0f)
        {
            var m = new Material(gearMat);
            m.SetColor("_Tint", new Color(r, g, b)); m.SetColor("_Side", new Color(r, g, b));
            m.SetFloat("_Metal", metal); m.SetFloat("_Rough", rough); m.SetFloat("_Aniso", aniso); m.SetFloat("_Mode", mode);
            return m;
        }

        Material Tex(Texture2D t, Color side, float metal, float rough)
        {
            var m = M(side.r, side.g, side.b, metal, rough);
            m.SetTexture("_MainTex", t); m.SetFloat("_UseTex", 1f);
            return m;
        }

        protected override void Build()
        {
            _club = Kit.Fullscreen(transform, clubMat, "club");
            _labels = new[] { label0, label1, label2, label3 };
            // Materials.
            _plateTT = Tex(ttTex, new Color(0.03f, 0.03f, 0.032f), 0.6f, 0.4f);
            _rubber = M(0.016f, 0.016f, 0.017f, 0f, 0.85f);
            _alu = M(0.78f, 0.79f, 0.81f, 1f, 0.3f, 0.7f);
            _aluDark = M(0.42f, 0.43f, 0.45f, 1f, 0.38f, 0.6f);
            _chrome = M(0.92f, 0.92f, 0.94f, 1f, 0.08f);
            _knob = M(0.022f, 0.022f, 0.024f, 0f, 0.5f, 0f, 3f);
            _knobLight = M(0.6f, 0.6f, 0.62f, 0.7f, 0.32f, 0f, 3f);
            _startBtn = Tex(startTex, new Color(0.55f, 0.56f, 0.58f), 1f, 0.35f);
            _vinyl = M(0.012f, 0.012f, 0.013f, 0f, 0.25f, 0.9f, 1f);
            _slip = Tex(slipTex, new Color(0.02f, 0.02f, 0.021f), 0f, 0.95f);
            _strobeM = M(0.5f, 0.5f, 0.5f, 0.8f, 0.35f, 0f, 2f);
            _led = M(0.03f, 0.03f, 0.03f, 0f, 0.3f);
            _meter = M(0f, 0f, 0f, 0f, 0.5f, 0f, 4f);
            _screen = M(0f, 0f, 0f, 0f, 0.5f, 0f, 5f); _screen.SetTexture("_MainTex", fxTex);
            _plateMix = Tex(mixTex, new Color(0.025f, 0.025f, 0.027f), 0.5f, 0.45f);
            _cap = M(0.03f, 0.03f, 0.032f, 0f, 0.45f);
            _line = M(0.8f, 0.8f, 0.8f, 0f, 0.5f);
            _cart = M(0.02f, 0.02f, 0.022f, 0f, 0.3f);
            _guard = M(0.95f, 0.38f, 0.04f, 0f, 0.35f);
            _table = M(0.03f, 0.027f, 0.025f, 0f, 0.5f);

            // Turntable meshes (units: 1 = 10 cm, plate top at y 0).
            _ttPlate = GearMesh.RoundedBox(new Vector3(4.53f, 0.15f, 3.53f), 0.02f, 3, "tt plate");
            _ttBody = GearMesh.RoundedBox(new Vector3(4.47f, 0.70f, 3.47f), 0.06f, 3, "tt body");
            _foot = GearMesh.Lathe(new[] { new Vector2(0f, 0f), new Vector2(0.36f, 0f), new Vector2(0.36f, 0f), new Vector2(0.36f, -0.36f), new Vector2(0.36f, -0.36f), new Vector2(0.31f, -0.42f), new Vector2(0f, -0.42f) }, 48, "foot");
            _platter = GearMesh.Lathe(new[] { new Vector2(0f, 0.10f), new Vector2(1.62f, 0.10f), new Vector2(1.62f, 0.10f), new Vector2(1.66f, 0.08f), new Vector2(1.66f, 0.08f), new Vector2(1.66f, -0.17f), new Vector2(1.66f, -0.17f), new Vector2(1.40f, -0.17f) }, 128, "platter");
            _band = GearMesh.Lathe(new[] { new Vector2(1.664f, 0.075f), new Vector2(1.664f, -0.16f) }, 256, "strobe band", 0f, true);
            _slipM = GearMesh.Lathe(new[] { new Vector2(0f, 0.13f), new Vector2(1.50f, 0.13f), new Vector2(1.50f, 0.13f), new Vector2(1.50f, 0.10f) }, 128, "slipmat");
            _record = GearMesh.Lathe(new[] { new Vector2(0.04f, 0.152f), new Vector2(1.47f, 0.15f), new Vector2(1.49f, 0.158f), new Vector2(1.51f, 0.152f), new Vector2(1.51f, 0.152f), new Vector2(1.51f, 0.13f) }, 160, "record");
            _spindle = GearMesh.Lathe(new[] { new Vector2(0f, 0.30f), new Vector2(0.025f, 0.295f), new Vector2(0.035f, 0.27f), new Vector2(0.035f, 0.27f), new Vector2(0.035f, 0.10f) }, 24, "spindle");
            _powerBody = GearMesh.Lathe(new[] { new Vector2(0f, 0.26f), new Vector2(0.15f, 0.26f), new Vector2(0.15f, 0.26f), new Vector2(0.155f, 0f) }, 48, "power body");
            _powerCap = GearMesh.Knob(0.13f, 0.13f, "power cap");
            _startM = GearMesh.RoundedBox(new Vector3(0.71f, 0.08f, 0.35f), 0.015f, 3, "start");
            _speedM = GearMesh.RoundedBox(new Vector3(0.46f, 0.05f, 0.11f), 0.012f, 3, "speed");
            _ledSmall = GearMesh.RoundedBox(new Vector3(0.05f, 0.012f, 0.025f), 0.004f, 2, "led");
            _target = GearMesh.Lathe(new[] { new Vector2(0.07f, 0.31f), new Vector2(0.075f, 0.30f), new Vector2(0.075f, 0.30f), new Vector2(0.08f, 0f) }, 32, "target light");
            _targetTip = GearMesh.Lathe(new[] { new Vector2(0f, 0.315f), new Vector2(0.06f, 0.31f), new Vector2(0.06f, 0.31f), new Vector2(0.06f, 0.29f) }, 32, "target tip");
            _pitchCap = GearMesh.RoundedBox(new Vector3(0.26f, 0.15f, 0.20f), 0.025f, 3, "pitch cap");
            _pitchLine = GearMesh.RoundedBox(new Vector3(0.22f, 0.004f, 0.016f), 0.001f, 1, "pitch line");
            // Tonearm.
            _ringM = GearMesh.Knob(0.47f, 0.10f, "arm ring");
            _baseAlu = GearMesh.Lathe(new[] { new Vector2(0f, 0.26f), new Vector2(0.28f, 0.26f), new Vector2(0.28f, 0.26f), new Vector2(0.30f, 0.10f), new Vector2(0.30f, 0.10f), new Vector2(0.30f, 0.0f) }, 48, "arm base");
            _post = GearMesh.Lathe(new[] { new Vector2(0f, ArmH), new Vector2(0.11f, ArmH), new Vector2(0.11f, ArmH), new Vector2(0.12f, 0.26f) }, 32, "arm post");
            _gimbal = GearMesh.RoundedBox(new Vector3(0.30f, 0.20f, 0.32f), 0.04f, 3, "gimbal");
            _hd = new Vector3(0.33f, 0f, 0.944f).normalized;      // the headshell angles in, toward the spindle
            _armC = new Vector3(0f, 0f, ArmLen) - _hd * 0.44f;
            var pts = new Vector3[24]; var rad = new float[24];
            for (int i = 0; i < 24; i++)
            {
                float u = i / 23f;
                float x = _armC.x * u - 0.16f * Mathf.Sin(Mathf.PI * u) + 0.07f * Mathf.Sin(2f * Mathf.PI * u);
                pts[i] = new Vector3(x, 0f, -0.62f + (_armC.z + 0.62f) * u);
                rad[i] = 0.042f;
            }
            _armTube = GearMesh.Tube(pts, rad, 16, "arm tube");
            _weight = GearMesh.Knob(0.17f, 0.32f, "counterweight");
            _headshell = GearMesh.RoundedBox(new Vector3(0.19f, 0.035f, 0.50f), 0.012f, 2, "headshell");
            _cartM = GearMesh.RoundedBox(new Vector3(0.17f, 0.19f, 0.27f), 0.02f, 2, "cartridge");
            _guardM = GearMesh.RoundedBox(new Vector3(0.14f, 0.05f, 0.10f), 0.015f, 2, "stylus guard");
            _lift = GearMesh.Tube(new[] { new Vector3(0f, 0f, 0f), new Vector3(0.12f, 0.02f, 0.02f), new Vector3(0.20f, 0.07f, 0.0f), new Vector3(0.23f, 0.12f, -0.03f) }, new[] { 0.012f, 0.012f, 0.012f, 0.012f }, 8, "finger lift");
            _collar = GearMesh.Knob(0.062f, 0.13f, "collar");
            _restPost = GearMesh.Lathe(new[] { new Vector2(0f, 0.36f), new Vector2(0.035f, 0.36f), new Vector2(0.035f, 0.36f), new Vector2(0.04f, 0f) }, 20, "rest post");
            _restCradle = GearMesh.RoundedBox(new Vector3(0.14f, 0.06f, 0.07f), 0.02f, 2, "rest cradle");
            _cueLever = GearMesh.RoundedBox(new Vector3(0.05f, 0.05f, 0.30f), 0.02f, 2, "cue lever");
            _adapter = GearMesh.Lathe(new[] { new Vector2(0f, 0.035f), new Vector2(0.21f, 0.035f), new Vector2(0.22f, 0.02f), new Vector2(0.22f, 0f) }, 48, "45 adapter");
            _hinge = GearMesh.RoundedBox(new Vector3(0.42f, 0.14f, 0.13f), 0.03f, 2, "hinge");
            // Mixer meshes (top at y 0).
            _mixBody = GearMesh.RoundedBox(new Vector3(3.33f, 1.0f, 4.14f), 0.03f, 3, "mixer");
            _knobS = GearMesh.Knob(0.075f, 0.15f, "knob s");
            _knobM = GearMesh.Knob(0.085f, 0.17f, "knob m");
            _knobL = GearMesh.Knob(0.10f, 0.18f, "knob l");
            _cueBtn = GearMesh.RoundedBox(new Vector3(0.16f, 0.06f, 0.10f), 0.02f, 2, "cue button");
            _faderCap = GearMesh.RoundedBox(new Vector3(0.20f, 0.15f, 0.26f), 0.03f, 3, "fader cap");
            _xfCap = GearMesh.RoundedBox(new Vector3(0.26f, 0.15f, 0.20f), 0.03f, 3, "xf cap");
            _faderLine = GearMesh.RoundedBox(new Vector3(0.17f, 0.004f, 0.016f), 0.001f, 1, "fader line");
            _meterQ = GearMesh.Quad(0.03f, 0.92f, "meter");
            _screenQ = GearMesh.Quad(0.50f, 0.25f, "fx screen");
            _fxBtn = GearMesh.RoundedBox(new Vector3(0.14f, 0.04f, 0.065f), 0.015f, 2, "fx button");
            _onOff = GearMesh.Lathe(new[] { new Vector2(0f, 0.07f), new Vector2(0.09f, 0.065f), new Vector2(0.10f, 0.05f), new Vector2(0.10f, 0.05f), new Vector2(0.10f, 0f) }, 32, "on off");
            _toggle = GearMesh.RoundedBox(new Vector3(0.05f, 0.05f, 0.045f), 0.012f, 2, "toggle");
            _tableM = GearMesh.RoundedBox(new Vector3(14f, 0.5f, 5.6f), 0.06f, 3, "table");

            _d[0].o = new Vector3(-DeckX, DeckY, DeckZ); _d[1].o = new Vector3(DeckX, DeckY, DeckZ);
            _d[0].recBpm = 124f; _d[1].recBpm = 126f;
            _d[0].label = 0; _d[1].label = 1;
            _d[0].on = true; _d[0].speed = 0.5556f; _d[0].phi = 38f; _d[0].groove = 1.3f;
            _mix = new Vector3(0f, MixY, 0f);
        }

        static Matrix4x4 TRS(Vector3 p, Quaternion q) => Matrix4x4.TRS(p, q, Vector3.one);
        static Matrix4x4 T(Vector3 p) => Matrix4x4.Translate(p);

        // ---------------------------------------------------------------- the mix

        static float Ease(float x) { x = Mathf.Clamp01(x); return x * x * (3f - 2f * x); }

        void Mix(float dt)
        {
            float bars = rx.beatS / 4f;
            int cyc = Mathf.FloorToInt(bars / 32f);
            float w = bars - cyc * 32f;                     // 0..32 through the mix
            int p = ((cyc % 2) + 2) % 2, n = 1 - p;        // playing and incoming deck
            var P = _d[p]; var N = _d[n];
            int chP = p == 0 ? 1 : 2, chN = p == 0 ? 2 : 1; // decks on channels 2 and 3

            // Faders, bass swap, cue.
            _fader[chP] = 1f - Ease((w - 28f) / 4f);
            _fader[chN] = Ease((w - 24f) / 4f);
            _fader[0] = 0f; _fader[3] = 0f;
            _low[chN] = w < 28f ? -1f : Mathf.Lerp(-1f, 0f, Ease((w - 28f) * 2f));
            _low[chP] = w < 28f ? 0f : Mathf.Lerp(0f, -1f, Ease((w - 28f) * 2f));
            _cue[chN] = w >= 20f && w < 28f; _cue[chP] = false;
            _xf = 0.5f;

            // Decks: START at 24 for the incoming deck, STOP at 32 (bar 0) for the outgoing one.
            bool nOn = w >= 24f;
            if (nOn != N.on) { N.on = nOn; N.press = _t; if (nOn) N.playBars = 0f; }
            if (!P.on && w < 24f) { P.on = true; P.press = _t; }
            // Needle drop on the incoming deck: lift 22.0, swing 22.25..23.25, lower 23.25..23.75.
            float nd = w - 22f;
            if (nd >= 0f && nd < 2f)
            {
                N.lift = nd < 0.25f ? Ease(nd / 0.25f) : nd < 1.25f ? 1f : 1f - Ease((nd - 1.25f) / 0.5f);
                if (nd >= 0.25f && nd < 1.25f) N.phi = Mathf.Lerp(N.phi, Phi(1.46f), 1f - Mathf.Exp(-dt * 6f));
                if (nd >= 1.25f) { N.phi = Phi(1.46f); N.groove = 1.46f; }
            }
            else if (w < 22f) N.lift = 0f;
            // The stopped deck gets a new record while nobody's looking.
            if (w >= 10f && w < 11f && !N.on) N.label = (cyc * 2 + n + 2) % _labels.Length;
            P.lift = 0f;
            foreach (var d in _d)
            {
                float rpm0 = 33.333f / 60f;
                d.pitch = Mathf.Clamp(rxBpm / d.recBpm - 1f, -0.08f, 0.08f);
                float target = d.on ? rpm0 * (1f + d.pitch) : 0f;
                d.speed = d.on ? Mathf.Min(target, d.speed + rpm0 * dt / 0.4f) : Mathf.Max(0f, d.speed - rpm0 * dt / 1.0f);
                d.ang += d.speed * 360f * dt;
                if (d.on && d.lift < 0.01f)
                {
                    d.playBars += dt * rxBpm / 240f;
                    d.groove = Mathf.Max(0.72f, d.groove - d.speed * dt * 0.0016f);
                    d.phi = Phi(d.groove);
                }
                for (int k = 0; k < 4; k++)
                {
                    float x = RowDots[k] * d.speed;
                    float f = RowDots[k] * rpm0 * (1f + RowPitch[k]);
                    float dr = x - f * Mathf.Round(x / f);
                    d.rows[k] = Mathf.Repeat(d.rows[k] + dr * dt, 1f);
                }
            }

            // The filter rides the build on the playing channel and snaps back on the drop.
            _color[chP] = Mathf.Lerp(_color[chP], rx.tension * 0.85f, 1f - Mathf.Exp(-dt * (rx.tension > _color[chP] ? 2f : 10f)));
            _color[chN] = Mathf.Lerp(_color[chN], 0f, 1f - Mathf.Exp(-dt * 4f));
            if (rx.dropped) _fxOn = 1f;
            _fxOn = Mathf.Max(0f, _fxOn - dt * rxBpm / 240f);

            // Meters: the music through each fader and EQ.
            float body = Mathf.Clamp01(0.32f + 0.5f * rx.bassFast + 0.35f * rx.midFast + 0.15f * rx.highFast);
            for (int c = 0; c < 4; c++)
            {
                float bassCut = 1f - 0.35f * Mathf.Clamp01(-_low[c]);
                float want = body * _fader[c] * bassCut * (1f - 0.25f * _color[c]);
                _meterV[c] = Eased.Follow(_meterV[c], want, 20f, 4f, dt);
            }
            float mst = Mathf.Max(_meterV[1], _meterV[2]) * 0.95f + 0.05f * rx.kick;
            _masterL = Eased.Follow(_masterL, mst * (0.96f + 0.04f * rx.Spec(0.2f)), 22f, 4f, dt);
            _masterR = Eased.Follow(_masterR, mst * (0.96f + 0.04f * rx.Spec(0.7f)), 22f, 4f, dt);
        }

        float rxBpm;

        /// The arm's angle from the pivot-spindle line that puts the stylus at radius r.
        static float Phi(float r)
        {
            float D = 2.15f, L = ArmLen;
            return Mathf.Acos(Mathf.Clamp((L * L + D * D - r * r) / (2f * L * D), -1f, 1f)) * Mathf.Rad2Deg;
        }

        Quaternion ArmYaw(Deck d)
        {
            var toSpindle = d.Centre - d.Pivot; toSpindle.y = 0f;
            float a0 = Mathf.Atan2(toSpindle.z, toSpindle.x) * Mathf.Rad2Deg;   // math angle in x/z
            float a = (a0 + d.phi) * Mathf.Deg2Rad;
            var f = new Vector3(Mathf.Cos(a), 0f, Mathf.Sin(a));
            return Quaternion.LookRotation(f, Vector3.up);
        }

        // ---------------------------------------------------------------- drawing

        void DrawDeck(Deck d, int idx)
        {
            var I = Quaternion.identity;
            GearDraw.Draw(_ttPlate, _plateTT, T(d.TT(226.5f, 176.5f, -7.5f)));
            GearDraw.Draw(_ttBody, _rubber, T(d.TT(226.5f, 176.5f, -50f)));
            foreach (var fz in new[] { new Vector2(45f, 45f), new Vector2(408f, 45f), new Vector2(45f, 308f), new Vector2(408f, 308f) })
            {
                var pr = GearDraw.Props; pr.SetFloat("_Spin", 100f);
                GearDraw.Draw(_foot, _knob, T(d.TT(fz.x, fz.y, -85f)), pr);
            }
            // Platter, strobe band, slipmat, record, spindle.
            var spin = Quaternion.Euler(0f, d.ang, 0f);
            var c = d.Centre;
            GearDraw.Draw(_platter, _aluDark, TRS(c, spin));
            {
                var pr = GearDraw.Props;
                var sd = (d.TT(32f, 82f) - c); sd.y = 0f; sd.Normalize();
                pr.SetVector("_StrobeDir", new Vector4(sd.x, sd.y, sd.z, strobe));
                pr.SetVector("_StrobeRows", new Vector4(d.rows[0], d.rows[1], d.rows[2], d.rows[3]));
                pr.SetFloat("_Smear", Mathf.Clamp01(d.speed / 0.15f));
                GearDraw.Draw(_band, _strobeM, TRS(c, spin), pr);
            }
            GearDraw.Draw(_slipM, _slip, TRS(c, spin));
            {
                var pr = GearDraw.Props;
                pr.SetTexture("_MainTex", _labels[d.label]);
                var yaw = ArmYaw(d);
                var piv = d.Pivot + Vector3.up * (ArmH + d.lift * 0.12f);
                pr.SetVector("_ArmA", new Vector4(piv.x, piv.y, piv.z, 1f));
                var tip = piv + yaw * _armC;
                pr.SetVector("_ArmB", new Vector4(tip.x, tip.y, tip.z, 1f));
                GearDraw.Draw(_record, _vinyl, TRS(c, spin), pr);
            }
            GearDraw.Draw(_spindle, _chrome, T(c));
            // Power knob / strobe illuminator, START-STOP, 33/45, target light.
            GearDraw.Draw(_powerBody, _alu, T(d.TT(32f, 82f)));
            {
                var pr = GearDraw.Props; pr.SetFloat("_Spin", 0.13f);
                GearDraw.Draw(_powerCap, _knob, TRS(d.TT(32f, 82f, 26f), Quaternion.Euler(0f, 40f, 0f)), pr);
            }
            float press = Mathf.Clamp01(1f - (_t - d.press) / 0.18f);
            GearDraw.Draw(_startM, _startBtn, T(d.TT(48.5f, 26.5f, 2f - 3f * press)));
            GearDraw.Draw(_speedM, _alu, T(d.TT(117f, 18f, 1.5f)));
            GearDraw.Draw(_speedM, _alu, T(d.TT(173f, 18f, 1.5f)));
            {
                var pr = GearDraw.Props; pr.SetColor("_Emit", new Color(3f, 0.15f, 0.08f) * leds);
                GearDraw.Draw(_ledSmall, _led, T(d.TT(138f, 18f, 4.2f)), pr);
            }
            GearDraw.Draw(_target, _alu, T(d.TT(262f, 22f)));
            {
                var pr = GearDraw.Props; pr.SetColor("_Emit", new Color(4f, 3.9f, 3.6f) * leds);
                GearDraw.Draw(_targetTip, _led, T(d.TT(262f, 22f)), pr);
            }
            // Pitch fader at the tempo: +8 % is pulled to the front.
            float pz = 127.5f - d.pitch * 100f * (167f / 16f);
            GearDraw.Draw(_pitchCap, _alu, T(d.TT(412.5f, pz, 7.5f)));
            GearDraw.Draw(_pitchLine, _cap, T(d.TT(412.5f, pz, 15.1f)));
            {
                var pr = GearDraw.Props; pr.SetColor("_Emit", new Color(0.2f, 3f, 0.4f) * leds * (Mathf.Abs(d.pitch) < 0.0005f ? 1f : 0.15f));
                GearDraw.Draw(_ledSmall, _led, TRS(d.TT(404f, 127.5f, 0.5f), Quaternion.Euler(0f, 90f, 0f)), pr);
            }
            // Tonearm.
            var pv = d.Pivot;
            {
                var pr = GearDraw.Props; pr.SetFloat("_Spin", 100f);
                GearDraw.Draw(_ringM, _knob, T(pv), pr);
            }
            GearDraw.Draw(_baseAlu, _alu, T(pv + Vector3.up * 0.10f * 0f));
            GearDraw.Draw(_post, _aluDark, T(pv));
            var yawQ = ArmYaw(d);
            var armO = pv + Vector3.up * (ArmH + d.lift * 0.12f);
            var armR = yawQ * Quaternion.Euler(-d.lift * 2.5f, 0f, 0f);
            var armM = TRS(armO, armR);
            GearDraw.Draw(_gimbal, _aluDark, armM * T(new Vector3(0f, 0.02f, 0f)));
            GearDraw.Draw(_armTube, _chrome, armM);
            {
                var pr = GearDraw.Props; pr.SetFloat("_Spin", 100f);
                GearDraw.Draw(_weight, _knob, armM * TRS(new Vector3(0f, 0f, -0.30f), Quaternion.Euler(-90f, 0f, 0f)), pr);
            }
            var hq = Quaternion.LookRotation(_hd, Vector3.up);
            var hsM = armM * TRS(_armC, hq);
            GearDraw.Draw(_collar, _alu, hsM * TRS(new Vector3(0f, 0f, -0.05f), Quaternion.Euler(90f, 0f, 0f)));
            GearDraw.Draw(_headshell, _alu, hsM * T(new Vector3(0f, -0.01f, 0.27f)));
            GearDraw.Draw(_lift, _alu, hsM * T(new Vector3(0.09f, -0.01f, 0.20f)));
            GearDraw.Draw(_cartM, _cart, hsM * T(new Vector3(0f, -0.125f, 0.32f)));
            GearDraw.Draw(_guardM, _guard, hsM * T(new Vector3(0f, -0.20f, 0.41f)));
            // Arm rest, cue lever (up when lifted), anti-skate, 45 adapter, hinges.
            GearDraw.Draw(_restPost, _aluDark, T(d.TT(392f, 197f)));
            GearDraw.Draw(_restCradle, _cap, TRS(d.TT(392f, 197f, 37f), Quaternion.Euler(0f, 14f, 0f)));
            GearDraw.Draw(_cueLever, _alu, TRS(d.TT(436f, 262f, 22f), Quaternion.Euler(-12f - 25f * d.lift, 0f, 0f)));
            {
                var pr = GearDraw.Props; pr.SetFloat("_Spin", 0.11f);
                GearDraw.Draw(_knobM, _knob, TRS(d.TT(412f, 232f), Quaternion.Euler(0f, -30f, 0f)) * Matrix4x4.Scale(new Vector3(1.3f, 0.6f, 1.3f)), pr);
            }
            GearDraw.Draw(_adapter, _alu, T(d.TT(45f, 318f)));
            GearDraw.Draw(_hinge, _cap, T(d.TT(70f, 352f, 2f)));
            GearDraw.Draw(_hinge, _cap, T(d.TT(330f, 352f, 2f)));
        }

        void Knob(Mesh m, Material mat, float r, float x, float z, float val, bool centred)
        {
            // val -1..1 (centred) or 0..1; the pointer sweeps -135..135 degrees.
            float a = centred ? val * 135f : -135f + 270f * Mathf.Clamp01(val);
            var pr = GearDraw.Props; pr.SetFloat("_Spin", r);
            GearDraw.Draw(m, mat, TRS(MX(x, z), Quaternion.Euler(0f, a, 0f)), pr);
        }

        void DrawMixer()
        {
            GearDraw.Draw(_mixBody, _plateMix, T(MX(166.5f, 207f, -50f)));
            for (int c = 0; c < 4; c++)
            {
                float cx = 76f + 45f * c + 22.5f;
                GearDraw.Draw(_toggle, _cap, T(MX(cx - 8f + 8f * (c == 0 || c == 3 ? 1 : 2), 387f, 2f)));
                Knob(_knobS, _knob, 0.075f, cx, 360f, 0.5f, false);
                Knob(_knobS, _knob, 0.075f, cx - 3f, 330f, 0f, true);
                Knob(_knobS, _knob, 0.075f, cx - 3f, 300f, 0f, true);
                Knob(_knobS, _knob, 0.075f, cx - 3f, 270f, _low[c], true);
                Knob(_knobL, _knobLight, 0.10f, cx, 235f, _color[c], true);
                {
                    var pr = GearDraw.Props;
                    float on = _cue[c] ? 1f : 0.04f;
                    pr.SetColor("_Emit", new Color(3.2f, 1.1f, 0.1f) * on * leds);
                    pr.SetColor("_Tint", new Color(0.25f, 0.12f, 0.03f));
                    GearDraw.Draw(_cueBtn, _led, T(MX(cx, 205f, 1f)), pr);
                }
                float fz = 70f + 100f * _fader[c];
                GearDraw.Draw(_faderCap, _cap, T(MX(cx, fz, 6f)));
                GearDraw.Draw(_faderLine, _line, T(MX(cx, fz, 13.6f)));
                GearDraw.Draw(_toggle, _cap, T(MX(cx, 52.5f, 2f)));
                {
                    var pr = GearDraw.Props; pr.SetFloat("_Level", _meterV[c]);
                    GearDraw.Draw(_meterQ, _meter, T(MX(cx + 14f, 307f, 0.2f)), pr);
                }
            }
            // Crossfader.
            float xx = 166f + (_xf - 0.5f) * 45f;
            GearDraw.Draw(_xfCap, _cap, T(MX(xx, 25f, 6f)));
            GearDraw.Draw(_faderLine, _line, TRS(MX(xx, 25f, 13.6f), Quaternion.Euler(0f, 90f, 0f)));
            // Left column.
            Knob(_knobS, _knob, 0.075f, 22f, 375f, 0.3f, false); Knob(_knobS, _knob, 0.07f, 54f, 375f, 0f, true);
            Knob(_knobS, _knob, 0.07f, 54f, 345f, 0f, true); Knob(_knobS, _knob, 0.07f, 22f, 345f, 0.5f, false);
            for (int i = 0; i < 6; i++)
            {
                float bx = 18f + (i % 3) * 20f, bz = 262f - (i / 3) * 16f;
                bool on = i == 5 && rx.tension > 0.2f;
                var pr = GearDraw.Props; pr.SetColor("_Emit", new Color(2.4f, 2.6f, 3f) * (on ? 1f : 0.03f) * leds);
                GearDraw.Draw(_fxBtn, _led, T(MX(bx, bz, 1f)), pr);
            }
            Knob(_knobM, _knob, 0.085f, 38f, 212f, 0.2f + 0.5f * rx.tension, true);
            Knob(_knobM, _knob, 0.085f, 38f, 150f, -0.6f, true);
            Knob(_knobM, _knob, 0.085f, 38f, 112f, 0.65f, false);
            // Right column.
            Knob(_knobM, _knob, 0.09f, 278f, 380f, 0.72f, false); Knob(_knobS, _knob, 0.07f, 312f, 380f, 0.4f, false);
            {
                var pr = GearDraw.Props; pr.SetFloat("_Level", _masterL);
                GearDraw.Draw(_meterQ, _meter, T(MX(284f, 296f, 0.2f)), pr);
                pr = GearDraw.Props; pr.SetFloat("_Level", _masterR);
                GearDraw.Draw(_meterQ, _meter, T(MX(300f, 296f, 0.2f)), pr);
            }
            {
                var pr = GearDraw.Props; pr.SetColor("_Emit", new Color(1f, 1f, 1f) * (0.9f + 1.4f * _fxOn) * leds);
                GearDraw.Draw(_screenQ, _screen, T(MX(292f, 209f, 0.3f)), pr);
            }
            for (int i = 0; i < 3; i++) GearDraw.Draw(_fxBtn, _cap, T(MX(275f + 17f * i, 182f, 1f)));
            Knob(_knobM, _knob, 0.085f, 278f, 148f, 0.4f, false); Knob(_knobS, _knob, 0.07f, 312f, 148f, 0.75f, false);
            Knob(_knobL, _knob, 0.10f, 292f, 106f, 0.2f, true);
            {
                var pr = GearDraw.Props;
                float on = 0.05f + _fxOn;
                pr.SetColor("_Emit", new Color(0.4f, 1.2f, 3.5f) * on * leds);
                pr.SetColor("_Tint", new Color(0.05f, 0.08f, 0.16f));
                GearDraw.Draw(_onOff, _led, T(MX(304f, 52f)), pr);
                pr = GearDraw.Props;
                pr.SetColor("_Emit", new Color(1.5f, 1.5f, 1.6f) * (Mathf.Repeat(rx.beatS, 1f) < 0.15f ? 1f : 0.05f) * leds);
                GearDraw.Draw(_fxBtn, _led, T(MX(272f, 50f, 1f)), pr);
            }
        }

        // ---------------------------------------------------------------- camera

        Vector3 Stylus(Deck d)
        {
            var piv = d.Pivot + Vector3.up * (ArmH + d.lift * 0.12f);
            return piv + ArmYaw(d) * (new Vector3(0f, -0.25f, ArmLen));
        }

        void Shot(int kind, Deck d, float u, out Vector3 pos, out Vector3 look, out float fstop)
        {
            float dr = drift;
            float sw = Mathf.Sin(u * Mathf.PI - Mathf.PI * 0.5f) * dr;   // -1..1 across the shot
            fstop = 5.6f;
            switch (kind)
            {
                default:
                case 0:   // the whole booth
                    look = new Vector3(0f, 1.1f, 0.3f);
                    pos = new Vector3(sw * 0.7f, 5.8f, -10.2f + 0.5f * u * dr);
                    fstop = 8f;
                    break;
                case 1:   // low across the record to the stylus and the arm
                {
                    var s = Stylus(d);
                    look = Vector3.Lerp(s, d.Pivot + Vector3.up * 0.3f, 0.35f);
                    pos = s + new Vector3(-1.75f - 0.25f * sw, 0.55f, -0.85f + 0.15f * sw);
                    break;
                }
                case 2:   // over the channel strips: EQs and meters
                    look = MX(166f, 300f);
                    pos = look + new Vector3(-0.7f * sw - 0.2f, 2.3f, -2.5f);
                    break;
                case 3:   // the strobe dots in the illuminator's light
                {
                    var c = d.Centre;
                    var sd = d.TT(32f, 82f) - c; sd.y = 0f; sd.Normalize();
                    var rim = c + sd * 1.66f;
                    look = rim + new Vector3(0.2f, 0f, 0.15f);
                    pos = rim + new Vector3(0.45f + 0.15f * sw, 0.38f, -1.15f);
                    break;
                }
                case 4:   // channel faders and crossfader
                    look = MX(166f, 100f);
                    pos = look + new Vector3(0.9f - 0.5f * sw, 1.9f, -2.3f);
                    break;
                case 5:   // above the spinning record
                    look = d.Centre;
                    pos = d.Centre + new Vector3(0.6f + 0.3f * sw, 4.0f, -1.9f);
                    fstop = 8f;
                    break;
                case 6:   // START-STOP and the speed buttons
                {
                    var b = d.TT(80f, 40f);
                    look = b;
                    pos = b + new Vector3(0.35f + 0.2f * sw, 0.95f, -1.35f);
                    break;
                }
            }
        }

        // ---------------------------------------------------------------- frame

        protected override void Frame(ShowState s, float dt)
        {
            dt = Mathf.Clamp(dt, 1e-4f, 1f / 20f);
            _t += dt;
            rxBpm = Mathf.Clamp(s.bpm, 60f, 200f);
            GearDraw.BeginFrame();
            Mix(dt);
            float gain = rx.Gain(0.9f, 1.15f);

            // Lights: the booth lamp, two club colours behind, a cool fill.
            var p1 = Kit.Hue(rx.beatS / 512f); var p2 = Kit.Hue(rx.beatS / 512f + 0.37f);
            float swing = Mathf.Sin(rx.phrase * 0.5f);
            _lp[0] = new Vector4(0.6f, 7.5f, -3.6f, 9f);  _lc[0] = (Vector4)(new Color(1f, 0.86f, 0.7f) * 2.3f * lamp);
            _lp[1] = new Vector4(-7f + 3f * swing, 5f, 9f, 9f); _lc[1] = (Vector4)(p1 * 1.6f * club * gain);
            _lp[2] = new Vector4(7f + 3f * swing, 5f, 9f, 9f);  _lc[2] = (Vector4)(p2 * 1.6f * club * gain);
            _lp[3] = new Vector4(-7f, 3f, -6f, 8f); _lc[3] = (Vector4)(new Color(0.35f, 0.45f, 0.7f) * 0.35f);
            Shader.SetGlobalVectorArray("_GLPos", _lp);
            Shader.SetGlobalVectorArray("_GLCol", _lc);
            Shader.SetGlobalVector("_GEnvA", (Vector4)(new Color(1f, 0.9f, 0.78f) * 0.5f * lamp));
            var e1 = p1 * 0.5f * club; var e2 = p2 * 0.5f * club;
            Shader.SetGlobalVector("_GEnvB", new Vector4(e1.r, e1.g, e1.b, 0.6f + swing * 0.5f));
            Shader.SetGlobalVector("_GEnvC", new Vector4(e2.r, e2.g, e2.b, -0.6f + swing * 0.5f));
            _club.SetFloat("_Swing", rx.clk * 0.08f);
            _club.SetFloat("_T", rx.clk);
            _club.SetFloat("_Hue", rx.beatS / 512f);
            _club.SetFloat("_Beams", club * (0.6f + 0.4f * gain));
            _club.SetFloat("_Wall", club);
            _club.SetFloat("_Gain", 1f);

            GearDraw.Draw(_tableM, _table, T(new Vector3(0f, TableY - 0.25f, 0.4f)));
            DrawDeck(_d[0], 0);
            DrawDeck(_d[1], 1);
            DrawMixer();

            // The shot: cut on bar lines, following the mix.
            float bars = rx.beatS / 4f;
            int cyc = Mathf.FloorToInt(bars / 32f);
            float w = bars - cyc * 32f;
            int p = ((cyc % 2) + 2) % 2;
            Deck P = _d[p], N = _d[1 - p];
            int kind; Deck deck; float s0, s1;
            if (w < 8f) { kind = 0; deck = P; s0 = 0f; s1 = 8f; }
            else if (w < 12f) { kind = 1; deck = P; s0 = 8f; s1 = 12f; }
            else if (w < 16f) { kind = 5; deck = P; s0 = 12f; s1 = 16f; }
            else if (w < 20f) { kind = 2; deck = P; s0 = 16f; s1 = 20f; }
            else if (w < 24f) { kind = 1; deck = N; s0 = 20f; s1 = 24f; }
            else if (w < 26f) { kind = 6; deck = N; s0 = 24f; s1 = 26f; }
            else if (w < 28f) { kind = 3; deck = N; s0 = 26f; s1 = 28f; }
            else { kind = 4; deck = P; s0 = 28f; s1 = 32f; }
            int shotId = cyc * 16 + Mathf.FloorToInt(s0);
            if (shotId != _shot) { _shot = shotId; rig.Snap(); }
            // START is pressed right on bar 24: open that shot half a beat early.
            float u = Mathf.Clamp01((w - s0) / (s1 - s0));
            Shot(kind, deck, u, out var pos, out var look, out float fstop);
            rig.Move(cam, pos, look, dt, 3f);
            HqPost.Focus(Vector3.Distance(cam.transform.position, look), fstop, 50f);
        }
    }
}
