// unity_speaker: close-ups of a club speaker stack: a top cabinet (15" woofer
// under a constant-directivity horn) on a sub (18" woofer, two flared bass
// ports), real proportions, no grilles. The music moves the speakers the way
// it really does:
//  - Shape: each kick drives the cones out and back (a quick push, a small
//    overshoot as they settle), the bass holds them forward a little; the
//    cone and dust cap move as one while the rubber surround stretches
//    between them and the fixed frame. The sub throws harder than the top.
//  - Motion: dust motes drift in the raking key light and are blown out of
//    the ports on every kick; the light swings with the phrase; the camera
//    moves slowly through macro shots, cutting every 8 bars.
// The cabinets never move or change shape: only the drivers do.
// Controls: excursion, drift, dust, key light, club lights.
using UnityEngine;

namespace TrippinStage
{
    public sealed class SpeakerShow : KitShow
    {
        public Material gearMat, clubMat, motesMat;

        [TrippinParam("Excursion", 0f, 2f)] public float excursion = 1f;
        [TrippinParam("Drift", 0f, 2f)] public float drift = 1f;
        [TrippinParam("Dust", 0f, 2f)] public float dust = 1f;
        [TrippinParam("Key light", 0.3f, 2f)] public float key = 1f;
        [TrippinParam("Club lights", 0f, 2f)] public float club = 1f;

        // Cabinets (units: 1 = 10 cm), front faces toward -z.
        static readonly Vector3 SubSize = new Vector3(6.0f, 6.5f, 7.0f), TopSize = new Vector3(4.8f, 7.5f, 4.5f);
        const float BaseY = 0f, HornY = BaseY + 6.5f + 5.85f;

        GearMats _m;
        Booth _booth;
        Material _cab, _cone, _surround, _cap, _frame, _hornM, _motes, _floor, _badge, _port;
        Mesh _throat, _corner, _floorM, _badgeM, _hornMesh, _lipH, _lipV, _portM;
        Cabinet _subCab, _topCab;
        Woofer _sub, _top;
        RenderParams _motesRp;
        float _k, _k2, _push;
        int _shot = -1;
        readonly Vector4[] _lp = new Vector4[4], _lc = new Vector4[4];

        /// A cabinet as it is built: side, top, bottom and back panels, and a
        /// baffle with the drivers' holes cut in it (so the cones sit inside).
        sealed class Cabinet
        {
            public Mesh side, cap, back, baffle;
            public Vector3 size, centre;
            public Cabinet(Vector3 size, Vector3 centre, Vector4[] holes, string name)
            {
                this.size = size; this.centre = centre;
                const float t = 0.18f;
                side = GearMesh.RoundedBox(new Vector3(t, size.y, size.z), 0.05f, 2, name + " side");
                cap = GearMesh.RoundedBox(new Vector3(size.x, t, size.z), 0.05f, 2, name + " top");
                back = GearMesh.RoundedBox(new Vector3(size.x, size.y, t), 0.05f, 2, name + " back");
                baffle = GearMesh.Baffle(size.x - 0.02f, size.y - 0.02f, holes, 0.04f, name + " baffle");
            }
        }

        sealed class Woofer
        {
            public Mesh frame, surround, cone, cap, bolt;
            public float rIn, rOut, rFrame, depth;
            public Vector3 c;         // centre of the baffle opening (world)
            public float exc;
        }

        public override bool UsesCamera => true;

        Woofer MakeWoofer(float inch, Vector3 centre)
        {
            float s = inch / 15f;
            var w = new Woofer { c = centre, rFrame = 1.95f * s, rOut = 1.75f * s, rIn = 1.50f * s, depth = 1.2f * s };
            // Surround: a half roll bulging forward (local +y is out of the baffle).
            var roll = new Vector2[17];
            for (int i = 0; i <= 16; i++)
            {
                float t = i / 16f;
                roll[i] = new Vector2(Mathf.Lerp(w.rIn, w.rOut, t), 0.14f * s * Mathf.Sin(Mathf.PI * t));
            }
            w.surround = GearMesh.Lathe(roll, 96, "surround");
            // Cone: from its rim back to the neck, slightly curved; dust cap dome on the neck.
            var cone = new Vector2[17];
            float rn = 0.55f * s;
            for (int i = 0; i <= 16; i++)
            {
                float t = i / 16f;                        // 0 rim .. 1 neck
                float r = Mathf.Lerp(w.rIn, rn, t);
                cone[i] = new Vector2(r, -w.depth * Mathf.Pow(t, 0.8f));
            }
            System.Array.Reverse(cone);                   // neck to rim, so the faces point out of the cabinet
            w.cone = GearMesh.Lathe(cone, 96, "cone");
            var cap = new Vector2[11];
            for (int i = 0; i <= 10; i++)
            {
                float a = i / 10f * Mathf.PI * 0.5f;
                cap[i] = new Vector2(rn * Mathf.Sin(a), -w.depth + 0.38f * s * Mathf.Cos(a) - 0.02f * s);
            }
            w.cap = GearMesh.Lathe(cap, 64, "dust cap");
            w.frame = GearMesh.Lathe(new[] { new Vector2(w.rOut - 0.02f, 0.06f), new Vector2(w.rFrame, 0.06f), new Vector2(w.rFrame, 0.06f), new Vector2(w.rFrame + 0.02f, 0f) }, 96, "frame");
            w.bolt = GearMesh.Lathe(new[] { new Vector2(0f, 0.12f), new Vector2(0.06f, 0.1f), new Vector2(0.075f, 0.06f), new Vector2(0.075f, 0.06f), new Vector2(0.075f, 0.0f) }, 16, "bolt");
            return w;
        }

        protected override void Build()
        {
            _m = new GearMats(gearMat);
            _booth = new Booth(transform, clubMat, _m) { drawTable = false };
            _cab = _m.M(0.045f, 0.045f, 0.048f, 0f, 0.7f);
            _cone = _m.M(0.07f, 0.07f, 0.075f, 0f, 0.75f, 0f, 9f);
            _surround = _m.M(0.03f, 0.03f, 0.032f, 0f, 0.45f);
            _cap = _m.M(0.025f, 0.025f, 0.027f, 0f, 0.15f);
            _frame = _m.M(0.04f, 0.04f, 0.045f, 0.4f, 0.5f);
            _hornM = _m.M(0.03f, 0.03f, 0.033f, 0f, 0.3f);
            _floor = _m.M(0.02f, 0.02f, 0.022f, 0f, 0.35f);
            _badge = _m.M(0.7f, 0.7f, 0.72f, 1f, 0.25f, 0.6f);
            _port = _m.M(0.01f, 0.01f, 0.011f, 0f, 0.7f);
            _motes = new Material(motesMat);

            _corner = GearMesh.RoundedBox(new Vector3(0.6f, 0.6f, 0.6f), 0.12f, 3, "corner");
            _floorM = GearMesh.RoundedBox(new Vector3(60f, 0.4f, 40f), 0.05f, 2, "stage floor");
            _badgeM = GearMesh.RoundedBox(new Vector3(0.9f, 0.25f, 0.06f), 0.03f, 2, "badge");
            _hornMesh = GearMesh.Horn(new Vector2(0.25f, 0.25f), new Vector2(2.0f, 1.1f), 2.4f, 1.7f, 28, "horn");
            _lipH = GearMesh.RoundedBox(new Vector3(4.4f, 0.16f, 0.12f), 0.04f, 2, "horn lip h");
            _lipV = GearMesh.RoundedBox(new Vector3(0.16f, 2.6f, 0.12f), 0.04f, 2, "horn lip v");
            // Deep end first, so the tube's faces point in (we look down the inside).
            _portM = GearMesh.Lathe(new[] { new Vector2(0.45f, -3.0f), new Vector2(0.45f, -0.3f), new Vector2(0.48f, -0.14f), new Vector2(0.55f, -0.04f), new Vector2(0.62f, 0.02f), new Vector2(0.62f, 0.02f), new Vector2(0.66f, 0f) }, 48, "port");
            _throat = GearMesh.RoundedBox(new Vector3(0.7f, 0.7f, 0.3f), 0.08f, 2, "throat");

            float subFront = -SubSize.z * 0.5f, topFront = -TopSize.z * 0.5f + 0.5f;
            _sub = MakeWoofer(18f, new Vector3(0f, BaseY + 3.6f, subFront));
            _top = MakeWoofer(15f, new Vector3(0f, BaseY + SubSize.y + 2.7f, topFront));
            var subC = new Vector3(0f, BaseY + SubSize.y * 0.5f, 0f);
            var topC = new Vector3(0f, BaseY + SubSize.y + TopSize.y * 0.5f, 0.5f);
            _subCab = new Cabinet(SubSize, subC, new[]
            {
                new Vector4(0f, _sub.c.y - subC.y, _sub.rOut + 0.05f, 0f),
                new Vector4(-2.2f, BaseY + 0.9f - subC.y, 0.5f, 0f), new Vector4(2.2f, BaseY + 0.9f - subC.y, 0.5f, 0f),
            }, "sub");
            _topCab = new Cabinet(TopSize, topC, new[]
            {
                new Vector4(0f, _top.c.y - topC.y, _top.rOut + 0.05f, 0f),
                new Vector4(0f, HornY - topC.y, 2.05f, -1.15f),
            }, "top");
            _motesRp = new RenderParams(_motes) { worldBounds = new Bounds(new Vector3(0f, 6f, -4f), Vector3.one * 30f) };
        }

        static Matrix4x4 T(Vector3 p) => GearDraw.T(p);
        static Matrix4x4 TRS(Vector3 p, Quaternion q) => GearDraw.TRS(p, q);
        static readonly Quaternion Out = Quaternion.Euler(-90f, 0f, 0f);   // local +y -> world -z

        void DrawWoofer(Woofer w)
        {
            var m = TRS(w.c, Out);
            var pr = GearDraw.Props;
            pr.SetFloat("_Exc", w.exc); pr.SetVector("_ExcR", new Vector4(w.rIn, w.rOut, 0f, 0f));
            GearDraw.Draw(w.cone, _cone, m, pr);
            pr = GearDraw.Props; pr.SetFloat("_Exc", w.exc); pr.SetVector("_ExcR", new Vector4(w.rIn, w.rOut, 0f, 0f));
            GearDraw.Draw(w.cap, _cap, m, pr);
            pr = GearDraw.Props; pr.SetFloat("_Exc", w.exc); pr.SetVector("_ExcR", new Vector4(w.rIn, w.rOut, 0f, 0f));
            GearDraw.Draw(w.surround, _surround, m, pr);
            GearDraw.Draw(w.frame, _frame, m);
            for (int i = 0; i < 8; i++)
            {
                float a = (i + 0.5f) / 8f * Mathf.PI * 2f;
                float r = (w.rOut + w.rFrame) * 0.5f;
                GearDraw.Draw(w.bolt, _frame, m * T(new Vector3(Mathf.Cos(a) * r, 0.06f, Mathf.Sin(a) * r)));
            }
        }

        void DrawCab(Cabinet c)
        {
            var size = c.size; var centre = c.centre;
            float t = 0.09f;
            GearDraw.Draw(c.side, _cab, T(centre + new Vector3(-size.x * 0.5f + t, 0f, 0f)));
            GearDraw.Draw(c.side, _cab, T(centre + new Vector3(size.x * 0.5f - t, 0f, 0f)));
            GearDraw.Draw(c.cap, _cab, T(centre + new Vector3(0f, size.y * 0.5f - t, 0f)));
            GearDraw.Draw(c.cap, _cab, T(centre + new Vector3(0f, -size.y * 0.5f + t, 0f)));
            GearDraw.Draw(c.back, _cab, T(centre + new Vector3(0f, 0f, size.z * 0.5f - t)));
            GearDraw.Draw(c.baffle, _cab, T(centre + new Vector3(0f, 0f, -size.z * 0.5f)));
            var h = size * 0.5f - Vector3.one * 0.22f;
            for (int i = 0; i < 8; i++)
            {
                var o = new Vector3((i & 1) == 0 ? -h.x : h.x, (i & 2) == 0 ? -h.y : h.y, (i & 4) == 0 ? -h.z : h.z);
                GearDraw.Draw(_corner, _frame, T(centre + o));
            }
        }

        void Shot(int kind, float u, out Vector3 pos, out Vector3 look, out float fstop)
        {
            float sw = Mathf.Sin(u * Mathf.PI - Mathf.PI * 0.5f) * drift;
            fstop = 5.6f;
            var portL = new Vector3(-2.2f, BaseY + 0.9f, -SubSize.z * 0.5f);
            switch (kind)
            {
                default:
                case 0:   // the stack
                    look = new Vector3(0f, 7.2f, -2f);
                    pos = new Vector3(4f * sw - 3f, 9f, -24f + 2f * u * drift);
                    fstop = 8f;
                    break;
                case 1:   // the sub's cone from low and to the side: the excursion reads
                    look = _sub.c + new Vector3(0.3f, 0f, -0.4f);
                    pos = _sub.c + new Vector3(-4.2f - 0.6f * sw, -1.6f, -4.6f);
                    break;
                case 2:   // the top: woofer and horn
                    look = _top.c + new Vector3(0f, 1.8f, -0.4f);
                    pos = look + new Vector3(2.6f + 0.8f * sw, 0.9f, -7.2f);
                    break;
                case 3:   // the dust cap, close
                    look = _sub.c + new Vector3(0f, 0f, -1.0f);
                    pos = look + new Vector3(1.3f + 0.4f * sw, 0.9f, -2.6f);
                    break;
                case 4:   // a port blowing dust
                    look = portL + new Vector3(0.2f, 0.2f, -0.8f);
                    pos = look + new Vector3(2.4f + 0.5f * sw, 1.4f, -3.6f);
                    break;
                case 5:   // along the surround's edge
                {
                    float a = Mathf.Deg2Rad * (35f + 20f * sw);
                    var rim = _sub.c + new Vector3(Mathf.Cos(a), Mathf.Sin(a), 0f) * (_sub.rOut - 0.1f) + new Vector3(0f, 0f, -0.15f);
                    look = rim;
                    pos = rim + new Vector3(-1.4f, -0.9f, -1.9f);
                    break;
                }
            }
        }

        protected override void Frame(ShowState s, float dt)
        {
            dt = Mathf.Clamp(dt, 1e-4f, 1f / 20f);
            GearDraw.BeginFrame();
            // The cones: a quick push on each kick, a little overshoot, the bass holding them out.
            _k = Eased.Follow(_k, rx.kick, 32f, 9f, dt);
            _k2 = Eased.Follow(_k2, _k, 10f, 10f, dt);
            float drive = excursion * (1f - 0.7f * rx.calm);
            _sub.exc = drive * (0.26f * _k - 0.09f * _k2 + 0.06f * rx.bassFast);
            _top.exc = drive * (0.08f * _k - 0.03f * _k2 + 0.05f * rx.midFast);
            _push = Eased.Follow(_push, _k * dust, 18f, 2.5f, dt);

            _booth.Frame(rx, 1f, club);
            // This stage's own lights: a raking warm key, two club colours behind, a low fill.
            float gain = rx.Gain(0.9f, 1.15f);
            var p1 = Kit.Hue(rx.beatS / 512f); var p2 = Kit.Hue(rx.beatS / 512f + 0.37f);
            float swing = Mathf.Sin(rx.phrase * 0.5f);
            var keyPos = new Vector3(-11f + 3f * swing, 15f, -8f);
            _lp[0] = new Vector4(keyPos.x, keyPos.y, keyPos.z, 22f); _lc[0] = (Vector4)(new Color(1f, 0.86f, 0.7f) * 5.5f * key);
            _lp[1] = new Vector4(-9f, 13f, 7f, 18f); _lc[1] = (Vector4)(p1 * 1.8f * club * gain);
            _lp[2] = new Vector4(9f, 11f, 7f, 18f); _lc[2] = (Vector4)(p2 * 1.8f * club * gain);
            _lp[3] = new Vector4(8f, 2f, -10f, 14f); _lc[3] = (Vector4)(new Color(0.35f, 0.45f, 0.7f) * 1.2f);
            Shader.SetGlobalVectorArray("_GLPos", _lp);
            Shader.SetGlobalVectorArray("_GLCol", _lc);

            GearDraw.Draw(_floorM, _floor, T(new Vector3(0f, BaseY - 0.2f, 0f)));
            DrawCab(_subCab);
            DrawCab(_topCab);
            DrawWoofer(_sub);
            DrawWoofer(_top);
            // Horn on the top cabinet, its lip on the baffle.
            var hornC = new Vector3(0f, HornY, -TopSize.z * 0.5f + 0.5f + 0.005f);
            GearDraw.Draw(_hornMesh, _hornM, T(hornC));
            GearDraw.Draw(_throat, _port, T(hornC + new Vector3(0f, 0f, 2.5f)));
            GearDraw.Draw(_lipH, _frame, T(hornC + new Vector3(0f, 1.22f, -0.06f)));
            GearDraw.Draw(_lipH, _frame, T(hornC + new Vector3(0f, -1.22f, -0.06f)));
            GearDraw.Draw(_lipV, _frame, T(hornC + new Vector3(2.12f, 0f, -0.06f)));
            GearDraw.Draw(_lipV, _frame, T(hornC + new Vector3(-2.12f, 0f, -0.06f)));
            // Ports on the sub, a badge on the top.
            foreach (var x in new[] { -2.2f, 2.2f })
                GearDraw.Draw(_portM, _port, TRS(new Vector3(x, BaseY + 0.9f, -SubSize.z * 0.5f - 0.01f), Out));
            GearDraw.Draw(_badgeM, _badge, T(new Vector3(1.6f, BaseY + SubSize.y + 0.45f, -TopSize.z * 0.5f + 0.5f - 0.04f)));

            // Dust in the key light, blown out of the ports.
            _motes.SetFloat("_T", Time.time);
            _motes.SetFloat("_Push", _push * 3f);
            _motes.SetFloat("_Size", 0.006f);
            _motes.SetFloat("_Gain", 0.35f * dust);
            _motes.SetVector("_BoxC", new Vector4(0f, 4.5f, -5.5f, 0f));
            _motes.SetVector("_BoxS", new Vector4(9f, 9f, 4f, 0f));
            _motes.SetVectorArray("_Port", new[] { new Vector4(-2.2f, BaseY + 0.9f, -SubSize.z * 0.5f, 0.45f), new Vector4(2.2f, BaseY + 0.9f, -SubSize.z * 0.5f, 0.45f) });
            _motes.SetVector("_KeyPos", keyPos);
            _motes.SetVector("_KeyAim", new Vector4(0f, 5f, -4f, 0f));
            if (dust > 0f) Graphics.RenderPrimitives(_motesRp, MeshTopology.Triangles, 900 * 6);

            // Shots: one per 8 bars, drifting.
            float bars = rx.beatS / 4f;
            int blk = Mathf.FloorToInt(bars / 8f);
            int[] order = { 0, 1, 3, 2, 4, 5 };
            int kind = order[((blk % order.Length) + order.Length) % order.Length];
            if (blk != _shot) { _shot = blk; rig.Snap(); }
            float u = Mathf.Repeat(bars, 8f) / 8f;
            Shot(kind, u, out var pos, out var look, out float fstop);
            rig.Move(cam, pos, look, dt, 3f);
            HqPost.Focus(Vector3.Distance(cam.transform.position, look), fstop, 50f);
        }
    }
}
