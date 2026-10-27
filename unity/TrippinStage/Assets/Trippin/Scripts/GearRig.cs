// The booth the DJ gear shows share (unity_decks, unity_cdj): the common
// materials, the four-channel club mixer with its part of the mix, the lights
// and the club behind, and the 32-bar mix clock both shows tell their story on.
using UnityEngine;

namespace TrippinStage
{
    /// Materials for every part, cloned from the HqGear base material.
    public sealed class GearMats
    {
        readonly Material _base;
        public readonly Material rubber, alu, aluDark, chrome, knob, knobLight, led, meter, cap, line, table, gloss;

        public GearMats(Material gearMat)
        {
            _base = gearMat;
            rubber = M(0.016f, 0.016f, 0.017f, 0f, 0.85f);
            alu = M(0.78f, 0.79f, 0.81f, 1f, 0.3f, 0.7f);
            aluDark = M(0.42f, 0.43f, 0.45f, 1f, 0.38f, 0.6f);
            chrome = M(0.92f, 0.92f, 0.94f, 1f, 0.08f);
            knob = M(0.022f, 0.022f, 0.024f, 0f, 0.5f, 0f, 3f);
            knobLight = M(0.6f, 0.6f, 0.62f, 0.7f, 0.32f, 0f, 3f);
            led = M(0.03f, 0.03f, 0.03f, 0f, 0.3f);
            meter = M(0f, 0f, 0f, 0f, 0.5f, 0f, 4f);
            cap = M(0.03f, 0.03f, 0.032f, 0f, 0.45f);
            line = M(0.8f, 0.8f, 0.8f, 0f, 0.5f);
            table = M(0.03f, 0.027f, 0.025f, 0f, 0.5f);
            gloss = M(0.02f, 0.02f, 0.022f, 0f, 0.18f);
        }

        public Material M(float r, float g, float b, float metal, float rough, float aniso = 0f, float mode = 0f)
        {
            var m = new Material(_base);
            m.SetColor("_Tint", new Color(r, g, b)); m.SetColor("_Side", new Color(r, g, b));
            m.SetFloat("_Metal", metal); m.SetFloat("_Rough", rough); m.SetFloat("_Aniso", aniso); m.SetFloat("_Mode", mode);
            return m;
        }

        public Material Tex(Texture2D t, Color side, float metal, float rough)
        {
            var m = M(side.r, side.g, side.b, metal, rough);
            m.SetTexture("_MainTex", t); m.SetFloat("_UseTex", 1f);
            return m;
        }
    }

    /// Where the 32-bar mix is: which deck is playing (p), which comes in (n),
    /// and how far through the mix we are (w, bars 0..32).
    public struct MixClock
    {
        public int cyc, p, n;
        public float w;
        public static MixClock At(Rx rx)
        {
            float bars = rx.beatS / 4f;
            var c = new MixClock { cyc = Mathf.FloorToInt(bars / 32f) };
            c.w = bars - c.cyc * 32f;
            c.p = ((c.cyc % 2) + 2) % 2; c.n = 1 - c.p;
            return c;
        }
        public static float Ease(float x) { x = Mathf.Clamp01(x); return x * x * (3f - 2f * x); }
    }

    /// A four-channel club mixer (layout in tools/gear_textures.py): the decks
    /// sit on channels 2 and 3. Update plays its part of the mix; Draw draws it.
    public sealed class ClubMixer
    {
        readonly GearMats _m;
        readonly Material _plate, _screen;
        readonly Mesh _body, _knobS, _knobM, _knobL, _cueBtn, _faderCap, _xfCap, _faderLine, _meterQ, _screenQ, _fxBtn, _onOff, _toggle;
        public Vector3 pos;
        public readonly float[] fader = new float[4], low = new float[4], color = new float[4], meterV = new float[4];
        public readonly bool[] cue = new bool[4];
        public float xf = 0.5f, masterL, masterR, fxOn;

        public Vector3 MX(float x, float z, float y = 0f) => pos + new Vector3((x - 166.5f) * 0.01f, y * 0.01f, (z - 207f) * 0.01f);

        public ClubMixer(GearMats m, Texture2D mixTex, Texture2D fxTex, Vector3 pos)
        {
            _m = m; this.pos = pos;
            _plate = m.Tex(mixTex, new Color(0.025f, 0.025f, 0.027f), 0.5f, 0.45f);
            _screen = m.M(0f, 0f, 0f, 0f, 0.5f, 0f, 5f); _screen.SetTexture("_MainTex", fxTex);
            _body = GearMesh.RoundedBox(new Vector3(3.33f, 1.0f, 4.14f), 0.03f, 3, "mixer");
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
        }

        /// The mixer's part of the mix: faders cross 24..32 with a bass swap at
        /// 28, the incoming channel is cued 20..28, the filter rides the build
        /// on the playing channel, BEAT FX fires for a bar on the drop, and the
        /// meters show the music through each fader and EQ.
        public void Update(Rx rx, MixClock mc, float dt, float bpm)
        {
            int chP = mc.p == 0 ? 1 : 2, chN = mc.p == 0 ? 2 : 1;
            float w = mc.w;
            fader[chP] = 1f - MixClock.Ease((w - 28f) / 4f);
            fader[chN] = MixClock.Ease((w - 24f) / 4f);
            fader[0] = 0f; fader[3] = 0f;
            low[chN] = w < 28f ? -1f : Mathf.Lerp(-1f, 0f, MixClock.Ease((w - 28f) * 2f));
            low[chP] = w < 28f ? 0f : Mathf.Lerp(0f, -1f, MixClock.Ease((w - 28f) * 2f));
            cue[chN] = w >= 20f && w < 28f; cue[chP] = false;
            xf = 0.5f;
            color[chP] = Mathf.Lerp(color[chP], rx.tension * 0.85f, 1f - Mathf.Exp(-dt * (rx.tension > color[chP] ? 2f : 10f)));
            color[chN] = Mathf.Lerp(color[chN], 0f, 1f - Mathf.Exp(-dt * 4f));
            if (rx.dropped) fxOn = 1f;
            fxOn = Mathf.Max(0f, fxOn - dt * bpm / 240f);
            float body = Mathf.Clamp01(0.32f + 0.5f * rx.bassFast + 0.35f * rx.midFast + 0.15f * rx.highFast);
            for (int c = 0; c < 4; c++)
            {
                float bassCut = 1f - 0.35f * Mathf.Clamp01(-low[c]);
                meterV[c] = Eased.Follow(meterV[c], body * fader[c] * bassCut * (1f - 0.25f * color[c]), 20f, 4f, dt);
            }
            float mst = Mathf.Max(meterV[1], meterV[2]) * 0.95f + 0.05f * rx.kick;
            masterL = Eased.Follow(masterL, mst * (0.96f + 0.04f * rx.Spec(0.2f)), 22f, 4f, dt);
            masterR = Eased.Follow(masterR, mst * (0.96f + 0.04f * rx.Spec(0.7f)), 22f, 4f, dt);
        }

        void Knob(Mesh mesh, Material mat, float r, float x, float z, float val, bool centred)
        {
            // val -1..1 (centred) or 0..1; the pointer sweeps -135..135 degrees.
            float a = centred ? val * 135f : -135f + 270f * Mathf.Clamp01(val);
            var pr = GearDraw.Props; pr.SetFloat("_Spin", r);
            GearDraw.Draw(mesh, mat, GearDraw.TRS(MX(x, z), Quaternion.Euler(0f, a, 0f)), pr);
        }

        public void Draw(Rx rx, float leds)
        {
            var knob = _m.knob; var cap = _m.cap; var led = _m.led;
            GearDraw.Draw(_body, _plate, GearDraw.T(MX(166.5f, 207f, -50f)));
            for (int c = 0; c < 4; c++)
            {
                float cx = 76f + 45f * c + 22.5f;
                GearDraw.Draw(_toggle, cap, GearDraw.T(MX(cx - 8f + 8f * (c == 0 || c == 3 ? 1 : 2), 387f, 2f)));
                Knob(_knobS, knob, 0.075f, cx, 360f, 0.5f, false);
                Knob(_knobS, knob, 0.075f, cx - 3f, 330f, 0f, true);
                Knob(_knobS, knob, 0.075f, cx - 3f, 300f, 0f, true);
                Knob(_knobS, knob, 0.075f, cx - 3f, 270f, low[c], true);
                Knob(_knobL, _m.knobLight, 0.10f, cx, 235f, color[c], true);
                {
                    var pr = GearDraw.Props;
                    pr.SetColor("_Emit", new Color(3.2f, 1.1f, 0.1f) * (cue[c] ? 1f : 0.04f) * leds);
                    pr.SetColor("_Tint", new Color(0.25f, 0.12f, 0.03f));
                    GearDraw.Draw(_cueBtn, led, GearDraw.T(MX(cx, 205f, 1f)), pr);
                }
                float fz = 70f + 100f * fader[c];
                GearDraw.Draw(_faderCap, cap, GearDraw.T(MX(cx, fz, 6f)));
                GearDraw.Draw(_faderLine, _m.line, GearDraw.T(MX(cx, fz, 13.6f)));
                GearDraw.Draw(_toggle, cap, GearDraw.T(MX(cx, 52.5f, 2f)));
                {
                    var pr = GearDraw.Props; pr.SetFloat("_Level", meterV[c]);
                    GearDraw.Draw(_meterQ, _m.meter, GearDraw.T(MX(cx + 14f, 307f, 0.2f)), pr);
                }
            }
            float xx = 166f + (xf - 0.5f) * 45f;
            GearDraw.Draw(_xfCap, cap, GearDraw.T(MX(xx, 25f, 6f)));
            GearDraw.Draw(_faderLine, _m.line, GearDraw.TRS(MX(xx, 25f, 13.6f), Quaternion.Euler(0f, 90f, 0f)));
            // Left column: mic, SOUND COLOR FX, headphones.
            Knob(_knobS, knob, 0.075f, 22f, 375f, 0.3f, false); Knob(_knobS, knob, 0.07f, 54f, 375f, 0f, true);
            Knob(_knobS, knob, 0.07f, 54f, 345f, 0f, true); Knob(_knobS, knob, 0.07f, 22f, 345f, 0.5f, false);
            for (int i = 0; i < 6; i++)
            {
                float bx = 18f + (i % 3) * 20f, bz = 262f - (i / 3) * 16f;
                bool on = i == 5 && rx.tension > 0.2f;
                var pr = GearDraw.Props; pr.SetColor("_Emit", new Color(2.4f, 2.6f, 3f) * (on ? 1f : 0.03f) * leds);
                GearDraw.Draw(_fxBtn, led, GearDraw.T(MX(bx, bz, 1f)), pr);
            }
            Knob(_knobM, knob, 0.085f, 38f, 212f, 0.2f + 0.5f * rx.tension, true);
            Knob(_knobM, knob, 0.085f, 38f, 150f, -0.6f, true);
            Knob(_knobM, knob, 0.085f, 38f, 112f, 0.65f, false);
            // Right column: master, meters, BEAT FX.
            Knob(_knobM, knob, 0.09f, 278f, 380f, 0.72f, false); Knob(_knobS, knob, 0.07f, 312f, 380f, 0.4f, false);
            {
                var pr = GearDraw.Props; pr.SetFloat("_Level", masterL);
                GearDraw.Draw(_meterQ, _m.meter, GearDraw.T(MX(284f, 296f, 0.2f)), pr);
                pr = GearDraw.Props; pr.SetFloat("_Level", masterR);
                GearDraw.Draw(_meterQ, _m.meter, GearDraw.T(MX(300f, 296f, 0.2f)), pr);
            }
            {
                var pr = GearDraw.Props; pr.SetColor("_Emit", new Color(1f, 1f, 1f) * (0.9f + 1.4f * fxOn) * leds);
                GearDraw.Draw(_screenQ, _screen, GearDraw.T(MX(292f, 209f, 0.3f)), pr);
            }
            for (int i = 0; i < 3; i++) GearDraw.Draw(_fxBtn, cap, GearDraw.T(MX(275f + 17f * i, 182f, 1f)));
            Knob(_knobM, knob, 0.085f, 278f, 148f, 0.4f, false); Knob(_knobS, knob, 0.07f, 312f, 148f, 0.75f, false);
            Knob(_knobL, knob, 0.10f, 292f, 106f, 0.2f, true);
            {
                var pr = GearDraw.Props;
                pr.SetColor("_Emit", new Color(0.4f, 1.2f, 3.5f) * (0.05f + fxOn) * leds);
                pr.SetColor("_Tint", new Color(0.05f, 0.08f, 0.16f));
                GearDraw.Draw(_onOff, led, GearDraw.T(MX(304f, 52f)), pr);
                pr = GearDraw.Props;
                pr.SetColor("_Emit", new Color(1.5f, 1.5f, 1.6f) * (Mathf.Repeat(rx.beatS, 1f) < 0.15f ? 1f : 0.05f) * leds);
                GearDraw.Draw(_fxBtn, led, GearDraw.T(MX(272f, 50f, 1f)), pr);
            }
        }
    }

    /// The booth's lights (a warm lamp over the gear, two club colours behind,
    /// a cool fill), the club behind (HqClub) and the table the gear sits on.
    public sealed class Booth
    {
        readonly Material _club, _table;
        readonly Mesh _tableM;
        readonly Vector4[] _lp = new Vector4[4], _lc = new Vector4[4];

        public Booth(Transform parent, Material clubMat, GearMats m)
        {
            _club = Kit.Fullscreen(parent, clubMat, "club");
            _table = m.table;
            _tableM = GearMesh.RoundedBox(new Vector3(14f, 0.5f, 5.6f), 0.06f, 3, "table");
        }

        public void Frame(Rx rx, float lamp, float club)
        {
            float gain = rx.Gain(0.9f, 1.15f);
            var p1 = Kit.Hue(rx.beatS / 512f); var p2 = Kit.Hue(rx.beatS / 512f + 0.37f);
            float swing = Mathf.Sin(rx.phrase * 0.5f);
            _lp[0] = new Vector4(0.6f, 7.5f, -3.6f, 9f); _lc[0] = (Vector4)(new Color(1f, 0.86f, 0.7f) * 2.3f * lamp);
            _lp[1] = new Vector4(-7f + 3f * swing, 5f, 9f, 9f); _lc[1] = (Vector4)(p1 * 1.6f * club * gain);
            _lp[2] = new Vector4(7f + 3f * swing, 5f, 9f, 9f); _lc[2] = (Vector4)(p2 * 1.6f * club * gain);
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
            GearDraw.Draw(_tableM, _table, GearDraw.T(new Vector3(0f, -0.25f, 0.4f)));
        }
    }
}
