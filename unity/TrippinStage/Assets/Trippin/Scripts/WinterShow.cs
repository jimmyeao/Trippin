// unity_winter: the HQ version of the 2D christmas scene: snow falling over
// an alpine village at night (an Alice backdrop), strings of coloured bulbs
// sagging across the foreground, through a 50 mm lens at f/5.6 that keeps
// most of it sharp, racking between the lights and the village every 8
// bars and opening up into bokeh in breakdowns.
//  - Shape: the bass blows gusts that swirl the snow round them (each kick
//    gives them a shove); the drop whirls it all up at once.
//  - Motion: the snow falls on the eased energy, the wind carries it
//    sideways, swinging round with the phrase; a chase runs along the
//    bulbs once a bar, stronger on the kicks; the camera eases sideways.
// Controls: snow, wind, gusts, bulbs, focus pull, flake size.
using UnityEngine;

namespace TrippinStage
{
    public sealed class WinterShow : KitShow
    {
        public Material snowMat, backMat;

        [TrippinParam("Snow", 0.2f, 1f)] public float amount = 1f;
        [TrippinParam("Wind", 0f, 3f)] public float wind = 1f;
        [TrippinParam("Gusts", 0f, 2f)] public float gusts = 1f;
        [TrippinParam("Bulbs", 0f, 2f)] public float bulbs = 1f;
        [TrippinParam("Focus pull", 0f, 1f)] public float pull = 1f;
        [TrippinParam("Flake size", 0.5f, 2.5f)] public float flake = 1f;

        public override bool UsesCamera => true;

        const int Flakes = 9000, Strands = 3, PerStrand = 44;
        Material _snow, _back;
        RenderParams _rp;
        readonly ClkAcc _fall = new ClkAcc(), _wind = new ClkAcc();
        readonly Vector4[] _gust = new Vector4[2];
        float _kickPrev, _gustAmt, _shove, _chase, _chaseAmt, _focus = 25f, _ap = 5.6f, _windX;

        protected override void Build()
        {
            Env(120f, 0f, false);
            _snow = new Material(snowMat);
            var b = new Bounds(new Vector3(0f, 8f, 30f), Vector3.one * 140f);
            _rp = new RenderParams(_snow) { worldBounds = b };
            _back = new Material(backMat);
            _back.SetFloat("_URange", 1f);
            _back.SetFloat("_EdgeFade", 0.1f);
            _back.SetFloat("_Haze", 0.05f);
            HqBackdrops.Make(transform, _back, null, new Vector3(0f, -38f, 75f), 200f, 200f * 768f / 1344f);
        }

        protected override void Frame(ShowState s, float dt)
        {
            dt = Mathf.Clamp(dt, 1e-4f, 1f / 20f);
            bool calm = rx.calm > 0.5f;
            float t = _fall.Step(rx.clk, 1f) * 0.75f;
            // The wind swings round with the phrase but the drift never jumps.
            float dir = Mathf.Sin(rx.phrase * 0.5f);
            _windX += dt * wind * (0.6f + 1.2f * dir) * (0.8f + 0.5f * rx.lum);

            bool kick = rx.kick > 0.55f && _kickPrev <= 0.55f && !calm;
            _kickPrev = rx.kick;
            if (kick) _shove = Mathf.Max(_shove, 0.5f);
            if (rx.dropped) _shove = 1.4f;
            _shove = Mathf.Max(0f, _shove - dt * 0.9f);
            _gustAmt = Eased.Follow(_gustAmt, calm ? 0f : rx.bassFast * 0.8f + _shove, 8f, 1.5f, dt);
            for (int g = 0; g < 2; g++)
            {
                float a = t * 0.07f + g * 3.1f;
                _gust[g] = new Vector4(Mathf.Sin(a) * 8f + (g == 0 ? -4f : 4f), 4f + Mathf.Sin(a * 1.3f) * 1.5f, 10f + g * 8f + Mathf.Cos(a * 0.8f) * 4f,
                                       gusts * _gustAmt * (g == 0 ? 1f : -0.8f));
            }

            // The chase: once round each string a bar, stronger on the kicks.
            _chase = Mathf.Repeat(rx.beatS / 4f, 1f);
            _chaseAmt = Eased.Follow(_chaseAmt, kick ? 1f : 0.25f, 30f, 2f, dt);

            float gain = rx.Gain(0.85f, 1.2f);
            _snow.SetFloat("_T", t);
            _snow.SetFloat("_Wind", _windX);
            _snow.SetVectorArray("_Gust", _gust);
            _snow.SetVector("_Chase", new Vector4(_chase, _chaseAmt, 0f, 0f));
            _snow.SetFloat("_Hue", Mathf.Repeat(rx.beatS / 512f, 1f));
            _snow.SetFloat("_Warmth", 1f);
            _snow.SetFloat("_Size", flake);
            _snow.SetFloat("_Gain", gain);
            int n = Mathf.RoundToInt(Flakes * amount);
            _snow.SetFloat("_Flakes", n);
            _snow.SetFloat("_Bulbs", bulbs);
            Graphics.RenderPrimitives(_rp, MeshTopology.Triangles, (n + (bulbs > 0f ? Strands * PerStrand : 0)) * 6);
            _back.SetFloat("_Gain", 1.1f * (0.9f + 0.15f * rx.lum));
            _back.SetFloat("_Lit", 0.8f + 0.6f * rx.highFast);

            // Focus racks between the bulbs and the village every 8 bars; a
            // breakdown settles on the snow drifting nearby.
            int block = Mathf.FloorToInt(rx.beatS / 32f);
            float want = ((block % 2) + 2) % 2 == 0 ? 8f : 20f;
            want = Mathf.Lerp(12f, want, pull);
            if (calm) want = Mathf.Lerp(12f, 5f, pull);
            _focus = Mathf.Lerp(_focus, want, 1f - Mathf.Exp(-dt * 1.2f));
            // A 50 mm lens at f/5.6 keeps most of the scene sharp; only a breakdown
            // opens it up to melt the far side into bokeh.
            _ap = Mathf.Lerp(_ap, calm ? Mathf.Lerp(5.6f, 2.8f, pull) : 5.6f, 1f - Mathf.Exp(-dt * 1.2f));
            HqPost.Focus(_focus, _ap, 50f);

            var camPos = new Vector3(Mathf.Sin(rx.phrase * 0.5f) * 1.2f, 1.7f, -2f);
            rig.Move(cam, camPos, camPos + new Vector3(0f, 0.6f, 20f), dt, 0.5f);
        }
    }
}
