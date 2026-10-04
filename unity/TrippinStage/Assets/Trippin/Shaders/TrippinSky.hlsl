// The horizon sky shared by Horizon.shader (the full-screen backdrop) and
// Land.shader (fog colour and the ocean's reflection), so the land fades into
// exactly the sky behind it. The including shader declares these in its
// UnityPerMaterial block:
//   float _SkyMode;   // 0 sun (synthwave dusk), 1 moon (night)
//   float _SkyHue, _SunH, _SunSize, _SkyGain, _SkyClk;
// _SunH is the sun/moon elevation (y of its direction), _SunSize its angular
// radius (radians), _SkyClk the smooth energy clock.
#ifndef TRIPPIN_SKY_INCLUDED
#define TRIPPIN_SKY_INCLUDED

float3 SkyPal(float t) { float3 c = TPalette(t); return c * c; }

float3 SkyStars(float3 d, float scale, float density, float tw)
{
    float3 p = d * scale;
    float3 cell = floor(p);
    float3 f = frac(p);
    float2 key = cell.xy + cell.z * float2(37.7, 17.3);
    float h = THash(key);
    float h2 = THash(key * 1.7 + 5.0);
    float3 c = 0.25 + 0.5 * float3(THash(key + 9.1), THash(key + 3.3), THash(key + 7.7));
    float size = lerp(0.05, 0.2, h2 * h2);
    float s = smoothstep(size, 0.0, length(f - c)) * step(density, h);
    float t = 1.0 + 0.35 * sin(tw * 0.6 + h * 60.0);
    return SkyPal(h2 * 0.5 + _SkyHue * 0.3) * s * t * (0.5 + 1.8 * h2);
}

float3 SkyCol(float3 d)
{
    float bass = _RxLvl.x, high = _RxLvl.w, lum = _RxMisc.y;
    float3 S = normalize(float3(0.0, _SunH, 1.0));
    float ang = acos(clamp(dot(d, S), -1.0, 1.0));
    float h = max(d.y, 0.0);
    float size = _SunSize * (1.0 + 0.12 * bass);
    float3 c;
    if (_SkyMode < 0.5)
    {
        float3 zen = SkyPal(_SkyHue + 0.65) * 0.05 + float3(0.004, 0.002, 0.015);
        float3 hor = SkyPal(_SkyHue) * 0.45;
        c = lerp(hor, zen, smoothstep(0.0, 0.55, h));
        // The sun, cut by scan bands in its lower half (the bands widen toward the bottom).
        float v = (d.y - S.y) / size;
        float disc = smoothstep(size, size * 0.985, ang);
        float cut = lerp(step(saturate(0.12 - v) * 0.8, frac(v * 8.0)), 1.0, step(0.12, v));
        float3 sunc = lerp(SkyPal(_SkyHue + 0.14) * 2.4, SkyPal(_SkyHue) * 2.0, saturate(0.5 - v * 0.5));
        c += sunc * disc * cut;
        c += SkyPal(_SkyHue + 0.08) * exp(-ang * 5.0) * 0.55 * (0.6 + bass);
        float cloud = smoothstep(0.52, 0.8, TFbm3(float3(d.x * 4.0 + _SkyClk * 0.0008, d.y * 16.0, 3.7))) * exp(-abs(h - 0.1) * 12.0);
        c += SkyPal(_SkyHue + 0.2) * cloud * 0.35 * (0.5 + lum);
        c += (SkyStars(d, 58.0, 0.96, _SkyClk) + 0.7 * SkyStars(d, 130.0, 0.94, _SkyClk + 9.0)) * smoothstep(0.12, 0.5, h) * (0.4 + high);
    }
    else
    {
        float3 zen = float3(0.002, 0.004, 0.012) + SkyPal(_SkyHue + 0.5) * 0.012;
        float3 hor = SkyPal(_SkyHue) * 0.12;
        c = lerp(hor, zen, smoothstep(0.0, 0.5, h));
        // The moon: a pale disc with maria, a halo, and a wider glow.
        float3 right = normalize(cross(float3(0, 1, 0), S));
        float3 up = cross(S, right);
        float3 rr = d - S * dot(d, S);
        float2 lp = float2(dot(rr, right), dot(rr, up)) / size;
        float disc = smoothstep(size, size * 0.98, ang);
        float maria = 0.72 + 0.28 * TFbm3(float3(lp * 2.2, 1.3));
        c += lerp(float3(1.0, 1.0, 1.0), SkyPal(_SkyHue + 0.1), 0.25) * 1.05 * maria * disc;
        c += SkyPal(_SkyHue + 0.05) * (exp(-ang * 5.0) * 0.22 + exp(-ang * 20.0) * 0.45) * (0.6 + bass);
        c += (SkyStars(d, 58.0, 0.93, _SkyClk) + 0.8 * SkyStars(d, 130.0, 0.9, _SkyClk + 9.0)) * smoothstep(0.03, 0.4, h) * (0.5 + 1.2 * high);
    }
    return c * _SkyGain;
}

#endif
