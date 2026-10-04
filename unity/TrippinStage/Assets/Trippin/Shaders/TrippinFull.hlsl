// Full-screen passes for the kit shows: a quad whose vertex shader writes clip
// space straight from uv, plus the world-space view ray through each pixel, so
// a fragment shader can raymarch / project without any scene geometry. Use
// ZTest Always + ZWrite Off and Queue Background, so it draws first and the
// additive show objects draw over it.
#ifndef TRIPPIN_FULL_INCLUDED
#define TRIPPIN_FULL_INCLUDED

struct FsA { float4 pos : POSITION; float2 uv : TEXCOORD0; };
struct FsV { float4 pos : SV_POSITION; float2 ndc : TEXCOORD0; float3 rd : TEXCOORD1; };

FsV FsVert(FsA i)
{
    FsV o;
    float2 ndc = i.uv * 2.0 - 1.0;
    o.pos = float4(ndc, 0.0, 1.0);
    o.ndc = ndc;
    // Un-project through the camera's own projection (not the GPU-flipped one);
    // _ProjectionParams.x is -1 when this camera renders with a flipped y.
    float vx = ndc.x / unity_CameraProjection[0][0];
    float vy = ndc.y / unity_CameraProjection[1][1] * _ProjectionParams.x;
    o.rd = UNITY_MATRIX_V[0].xyz * vx + UNITY_MATRIX_V[1].xyz * vy - UNITY_MATRIX_V[2].xyz;
    return o;
}

#endif
