// unity_ink: the HQ version of the 2D ink scene: drops of coloured ink
// falling into a dark tank of water (HqInk.shader, ray marched): each forms
// a vortex ring that sinks and spreads, trailing its stem, billowing into
// soft clouds as it ages.
//  - Shape: the kick on each bar's one drops new ink at a new spot (the
//    bass sets the drop's size); the bass swells the billows; the drop lets
//    three fall at once; on the breakdown one slow drop every two bars.
//  - Motion: the ink's clock runs on the eased energy, so the rings sink
//    and billow faster as the track drives; the view drifts slowly round.
// Controls: drop size, density, billow, glow, ink speed, colour.
using UnityEngine;

namespace TrippinStage
{
    public sealed class InkShow : KitShow
    {
        public Material inkMat;

        [TrippinParam("Drop size", 0.5f, 1.8f)] public float dropSize = 1f;
        [TrippinParam("Density", 0.3f, 2.5f)] public float density = 1f;
        [TrippinParam("Billow", 0f, 2.5f)] public float billow = 1f;
        [TrippinParam("Glow", 0.3f, 2.5f)] public float glow = 1f;
        [TrippinParam("Ink speed", 0.3f, 2.5f)] public float inkSpeed = 1f;
        [TrippinParam("Colour", 0f, 1f)] public float hue = 0f;

        public override bool UsesCamera => true;

        Material _m;
        const int Slots = 8;
        readonly Vector4[] _drop = new Vector4[Slots], _col = new Vector4[Slots];
        readonly Vector4[] _pd = new Vector4[Slots], _pc = new Vector4[Slots], _ps = new Vector4[Slots];
        float _t, _kickPrev, _beatPrev = -1f, _swell;
        int _barPrev = int.MinValue;
        int _drops;

        protected override void Build()
        {
            _m = Kit.Fullscreen(transform, inkMat, "ink");
            _m.SetTexture("_Vol", HqNoise.Volume);
            for (int i = 0; i < Slots; i++) _drop[i] = new Vector4(0f, 0f, 99f, 1f);
        }

        void Drip(float size)
        {
            int best = 0;
            for (int i = 1; i < Slots; i++) if (_drop[i].z > _drop[best].z) best = i;      // the oldest makes way
            _drops++;
            _drop[best] = new Vector4((Mathf.Repeat(_drops * 0.618034f, 1f) - 0.5f) * 13f, (Kit.H(_drops, 2) - 0.5f) * 5f, 0.001f, size * dropSize);
            Color c = Kit.Hue(hue + _drops * 0.21f);
            Color.RGBToHSV(c, out float h, out float s, out float v);
            c = Color.HSVToRGB(h, Mathf.Max(s, 0.85f), 1f);
            _col[best] = new Vector4(c.r, c.g, c.b, 1f);
        }

        protected override void Frame(ShowState s, float dt)
        {
            dt = Mathf.Clamp(dt, 1e-4f, 1f / 20f);
            bool calm = rx.calm > 0.5f;
            float rate = inkSpeed * (calm ? 0.45f : 0.7f + 0.6f * rx.lum);
            for (int i = 0; i < Slots; i++) if (_drop[i].z < 90f) _drop[i].z += dt * rate;
            _t += dt * rate;

            bool kick = rx.kick > 0.55f && _kickPrev <= 0.55f && !calm;
            _kickPrev = rx.kick;
            // One drop a bar, on the first kick of it, or the tank clogs.
            int bar = Mathf.FloorToInt(rx.beatS / 4f);
            if (kick && bar != _barPrev) { _barPrev = bar; Drip(0.8f + 0.5f * rx.bassFast); }
            float b8 = rx.beatS / 8f;
            if (calm && _beatPrev >= 0f && Mathf.Floor(b8) != Mathf.Floor(_beatPrev)) Drip(1.2f);
            _beatPrev = b8;
            if (rx.dropped) { Drip(1.3f); Drip(1.1f); Drip(1.2f); }

            // Pack the live drops first, with their shapes worked out here.
            int n = 0;
            for (int i = 0; i < Slots; i++)
            {
                var d = _drop[i];
                if (d.z <= 0f || d.z > 11f) continue;
                float age = d.z, sz = d.w;
                float depth = 5.8f * (1f - Mathf.Exp(-age * 0.42f)) * sz;
                _pd[n] = d;
                _pc[n] = _col[i];
                _ps[n] = new Vector4(9f - 0.3f - depth, (0.35f + 0.9f * (1f - Mathf.Exp(-age * 0.3f))) * sz, (0.16f + 0.06f * age) * sz, Mathf.Max(age - 0.8f, 0f) * 0.5f * sz);
                n++;
            }
            _m.SetVectorArray("_Drop", _pd);
            _m.SetVectorArray("_DropCol", _pc);
            _m.SetVectorArray("_Shape", _ps);
            _m.SetInt("_Count", n);
            _m.SetFloat("_T", _t);
            _m.SetFloat("_Density", density);
            _swell = Eased.Follow(_swell, rx.bassFast, 14f, 3f, dt);
            _m.SetFloat("_Billow", billow * (0.85f + 0.5f * _swell));
            _m.SetFloat("_Glow", glow * 1.9f);
            _m.SetVector("_Light", new Vector4(1f, 0.97f, 0.92f, 0f));
            _m.SetFloat("_Gain", rx.Gain(0.85f, 1.2f));

            float a = Mathf.Sin(rx.phrase * 0.5f) * 0.35f;
            var pos = new Vector3(Mathf.Sin(a) * 12f, 3.6f, -Mathf.Cos(a) * 12f);
            rig.Move(cam, pos, new Vector3(0f, 4.6f, 0f), dt, 0.5f);
        }
    }
}
