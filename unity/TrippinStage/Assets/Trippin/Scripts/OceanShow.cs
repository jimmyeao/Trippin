// unity_ocean: a night sea under a huge moon, with searchlights sweeping the
// sky from the horizon, seen skimming the surface. The Land shader draws the
// water (an opaque lit surface reflecting the sky, not a wireframe) and the
// Horizon shader the moon and stars.
//  - Shape: eight swells, each driven by its own slice of the eased spectrum:
//    the long swells follow the bass and the fine chop follows the highs, so
//    the surface is a live spectrum; every kick launches a ripple that spreads
//    across the water; crests glow with the mids and foam with the highs; the
//    moon swells with the bass.
//  - Motion: the waves travel on the smooth energy clock; the wind direction
//    (and so the swell) turns with the phrase; the searchlights sweep and
//    reverse with the phrase.
//  - Luminance: the water, moon and searchlights follow the music's loudness.
//  - Drops: a build calms the sea dim; the drop heaves it up and throws a ring
//    (eased).
using UnityEngine;

namespace TrippinStage
{
    public sealed class OceanShow : KitShow
    {
        Material _land, _sky;
        BeamPool _lights;
        readonly float[] _ringR = { -1f, -1f, -1f, -1f };
        readonly float[] _ringK = new float[4];
        bool _armed = true, _wasDrop;

        protected override void Build()
        {
            _sky = Kit.Fullscreen(transform, horizonMat, "night sky");
            _sky.SetFloat("_SkyMode", 1f);
            _sky.SetFloat("_SunH", 0.3f);
            _sky.SetFloat("_SunSize", 0.09f);
            _land = new Material(landMat);
            _land.SetFloat("_Mode", 1f);
            _land.SetFloat("_SizeX", 320f);
            _land.SetFloat("_SizeZ", 300f);
            _land.SetFloat("_Fade", 130f);
            _land.SetFloat("_SunH", 0.3f);
            _land.SetFloat("_SunSize", 0.09f);
            _land.SetFloat("_SkyMode", 1f);
            Kit.Part(transform, "ocean", Kit.GridMesh(256, 256, "ocean"), _land, new Vector3(0f, 0f, 150f), Vector3.one);
            _lights = new BeamPool(transform, beamMat, 4, "searchlight", false, 0.25f, 0.02f, 10f, 0.7f, 0.5f, 0.2f);
        }

        void Spawn()
        {
            int best = 0; float oldest = -2f;
            for (int i = 0; i < 4; i++)
            {
                float age = _ringR[i] < 0f ? 9f : _ringR[i];
                if (age > oldest) { oldest = age; best = i; }
            }
            _ringR[best] = 0.03f;
            _ringK[best] = 1f;
        }

        void Sky(Material m, float gain)
        {
            m.SetFloat("_SkyHue", 0.55f + 0.08f * Mathf.Sin(rx.phrase * 0.5f));
            m.SetFloat("_SkyGain", gain);
            m.SetFloat("_SkyClk", rx.clk);
        }

        protected override void Frame(ShowState s, float dt)
        {
            float gain = rx.Gain(0.5f, 1.5f);
            Sky(_sky, gain);
            Sky(_land, gain);
            if (rx.kick > 0.55f && _armed) { Spawn(); _armed = false; }
            else if (rx.kick < 0.3f) _armed = true;
            if (rx.dropped && !_wasDrop) Spawn();
            _wasDrop = rx.dropped;
            for (int i = 0; i < 4; i++)
            {
                if (_ringR[i] < 0f) { _ringK[i] = 0f; continue; }
                _ringR[i] += dt * 0.14f;
                if (_ringR[i] > 1f) { _ringR[i] = -1f; _ringK[i] = 0f; continue; }
                _ringK[i] = Mathf.Pow(1f - _ringR[i], 1.5f);
            }
            _land.SetVector("_RingR", new Vector4(_ringR[0], _ringR[1], _ringR[2], _ringR[3]));
            _land.SetVector("_RingK", new Vector4(_ringK[0], _ringK[1], _ringK[2], _ringK[3]));
            _land.SetFloat("_Gain", gain);
            _land.SetFloat("_Amp", (1.2f + 1.0f * rx.bassSlow + 0.9f * rx.bassFast + 1.0f * rx.impact) * (1f - 0.4f * rx.tension));
            _land.SetFloat("_Phase", rx.clk * 0.3f);
            _land.SetVector("_P0", new Vector4(0.9f + 0.6f * Mathf.Sin(rx.phrase), 0f, 0f, 0f));
            for (int i = 0; i < 4; i++)
            {
                float side = i < 2 ? -1f : 1f;
                var from = new Vector3(side * (50f + 20f * (i % 2)), 0.5f, 240f);
                float sweep = Mathf.Sin(rx.phrase * 0.5f + i * 1.4f) * 30f * (1f + 0.4f * rx.highFast);
                var to = from + new Vector3(sweep - side * 30f, 130f, -10f);
                _lights.Set(i, from, to, Kit.Hue(0.5f + i * 0.08f), gain * (0.4f + 0.8f * rx.midFast));
            }
            float sway = Mathf.Sin(rx.phrase * 0.5f);
            rig.Move(cam, new Vector3(5f * sway, 7f, 0f), new Vector3(2f * sway, 9f, 100f), dt, 1f);
        }
    }
}
