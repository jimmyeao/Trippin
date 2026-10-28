// The fog-machine smoke unity_lasers hangs over the stage, shared by the
// light volume (LaserLight.compute bakes it for the smoke pass) and the beams
// (Beam.shader with _SmokeVol 1), so a beam shows brightest exactly where a
// cloud is. Plain HLSL objects so a compute shader can include it too. Set by
// LasersShow (globals, and on the compute shader):
//   _LSVol    the baked noise volume (HqNoise.Volume)
//   _LSShape  x: texture scale (1/m), y: height falloff (m), zw: threshold lo/hi
//   _LSWind   xyz: how far the smoke has drifted (m), w: overall density
#ifndef TRIPPIN_LASER_SMOKE_INCLUDED
#define TRIPPIN_LASER_SMOKE_INCLUDED

Texture3D<float4> _LSVol; SamplerState sampler_LSVol;
float4 _LSShape, _LSWind;

float LSDensity(float3 p)
{
    float3 q = (p - _LSWind.xyz) * _LSShape.x;
    // Banks hug the floor and thin out with height.
    float h = exp(-max(p.y, 0.0) / _LSShape.y);
    float n = _LSVol.SampleLevel(sampler_LSVol, q, 0).r * 0.62;
    // The fine octave adds at most 0.38: skip it where even that can't reach the threshold.
    if (n + 0.38 <= _LSShape.z || h < 0.02) return 0.0;
    n += _LSVol.SampleLevel(sampler_LSVol, q * 2.7 + 0.37, 0).g * 0.38;
    return saturate((n - _LSShape.z) / (_LSShape.w - _LSShape.z)) * h * _LSWind.w;
}

#endif
