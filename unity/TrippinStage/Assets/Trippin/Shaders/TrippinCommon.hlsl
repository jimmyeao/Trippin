// Shared globals from TrippinLink.PushGlobals — the same audio vocabulary
// Trippin's WGSL scenes use (shaders/common.wgsl).
#ifndef TRIPPIN_COMMON_INCLUDED
#define TRIPPIN_COMMON_INCLUDED

float _TBeat, _TBeatPhase, _TBarPhase, _TKick, _TOnset, _TEnergy, _TIntensity, _TCalm, _TFlow;
float4 _TLvl, _THits, _TPres, _TClock;
float4 _TPal[8];
float4 _TSpec[8];

// The kit shows' eased signals (Rx.PushGlobals in Kit.cs): fast attack, slower
// release, so shape and brightness follow the music without stepping.
float4 _RxLvl;   // bass, mid, mid-high, high (eased levels)
float4 _RxMisc;  // kick, loudness, tension (build), impact (eased drop)
float4 _RxClk;   // smooth energy clock (beats), high clock, phrase angle, calm
float4 _RxSpec[8];

// Eased spectrum at x in 0..1, linearly interpolated between the 32 bins.
float RxSpec(float x)
{
    float f = saturate(x) * 31.0;
    int i = min((int)f, 30);
    float a = _RxSpec[i >> 2][i & 3];
    float b = _RxSpec[(i + 1) >> 2][(i + 1) & 3];
    return lerp(a, b, f - i);
}

// Palette across the 8 feed colours, t in 0..1 (wraps).
float3 TPalette(float t)
{
    t = frac(t) * 7.0;
    int i = min((int)t, 6);
    return lerp(_TPal[i].rgb, _TPal[i + 1].rgb, t - i);
}

// Spectrum band 0..31.
float TSpectrum(float x)
{
    int i = clamp((int)(saturate(x) * 31.0), 0, 31);
    float4 v = _TSpec[i >> 2];
    int j = i & 3;
    return j == 0 ? v.x : (j == 1 ? v.y : (j == 2 ? v.z : v.w));
}

// Beat pulse that relaxes in breakdowns (calm-aware, like beat_pulse()).
float TBeatPulse(float sharp)
{
    return lerp(exp(-_TBeatPhase * sharp), 0.25, _TCalm);
}

// PCG-style integer hash → 0..1.
float THash(float2 p)
{
    uint2 v = (uint2)(int2)floor(p) * uint2(1597334673u, 3812015801u);
    uint n = (v.x ^ v.y) * 1597334673u;
    n ^= n >> 16; n *= 2246822519u; n ^= n >> 13;
    return (n & 0xffffffu) / 16777216.0;
}

float TNoise(float2 p)
{
    float2 i = floor(p), f = frac(p);
    f = f * f * (3.0 - 2.0 * f);
    float a = THash(i), b = THash(i + float2(1, 0)), c = THash(i + float2(0, 1)), d = THash(i + float2(1, 1));
    return lerp(lerp(a, b, f.x), lerp(c, d, f.x), f.y);
}

float TNoise3(float3 p)
{
    // Two offset 2D layers blended along z — cheap and good enough for haze.
    float z = floor(p.z), f = frac(p.z);
    f = f * f * (3.0 - 2.0 * f);
    float a = TNoise(p.xy + z * float2(17.1, 31.7));
    float b = TNoise(p.xy + (z + 1.0) * float2(17.1, 31.7));
    return lerp(a, b, f);
}

float TFbm3(float3 p)
{
    float s = 0.0, a = 0.5;
    for (int i = 0; i < 4; i++) { s += a * TNoise3(p); p = p * 2.03 + 11.7; a *= 0.5; }
    return s;
}

#endif
