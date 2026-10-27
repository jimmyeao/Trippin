// unity_fireflies' fireflies: a few thousand points of light worked out
// from their index in the vertex shader (no stored state). Each has a home
// in the forest, wanders round it on slow sines, and is drawn toward one of
// three drifting swarm centres as _Pull rises (the bass gathers them).
// Each glows faintly and blinks on its own; a flash wave (_Wave: centre,
// age) ripples outward through the swarm, lighting each firefly as the
// front passes (synchronous fireflies). They are small opaque, depth-
// writing discs, so the show's depth of field blurs each by its true
// distance into a bokeh disc and bloom gives them their glow.
//  _T        the wander and blink clock (eased energy)
//  _Swarm[3] the swarm centres; _Pull how far each firefly is drawn in
//  _Lift     raises the swarm (the mids)
//  _PalMix   0 natural firefly green-gold .. 1 the palette
Shader "Trippin/HqFirefly"
{
    Properties
    {
        _Gain ("Gain", Float) = 1
        _Size ("Size", Float) = 1
        _PalMix ("Palette mix", Float) = 0
    }
    SubShader
    {
        Tags { "RenderType"="Opaque" "Queue"="Geometry" "RenderPipeline"="UniversalPipeline" }
        Cull Off

        HLSLINCLUDE
        #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/Core.hlsl"
        #include "TrippinCommon.hlsl"

        float _Gain, _Size, _PalMix, _T, _Pull, _Lift;
        float4 _Swarm[3];
        float4 _Wave[4];

        struct V { float4 pos : SV_POSITION; float2 q : TEXCOORD0; float3 col : TEXCOORD1; };

        uint FHash(uint x)
        {
            x ^= x >> 16; x *= 0x7feb352du; x ^= x >> 15; x *= 0x846ca68bu; x ^= x >> 16;
            return x;
        }
        float R(uint i, uint salt) { return (FHash(i * 9781u + salt * 6271u) & 0xffffff) / 16777216.0; }

        V Fly(uint vid)
        {
            V o;
            uint id = vid / 6, c = vid % 6;
            float2 corner[6] = { float2(-1,-1), float2(1,-1), float2(-1,1), float2(-1,1), float2(1,-1), float2(1,1) };
            float2 q = corner[c];
            o.q = q;

            float u1 = R(id, 1), u2 = R(id, 2), u3 = R(id, 3), u4 = R(id, 4), u5 = R(id, 5), u6 = R(id, 6);
            // Home: spread through the clearing, wider further away.
            float z = 3.0 + 55.0 * u3 * u3;
            float3 home = float3((u1 - 0.5) * (6.0 + z * 1.3), 0.4 + 8.0 * u2 * u2 + _Lift * (0.5 + u2), z);
            // Wander on slow sines, each its own.
            float t = _T * (0.35 + 0.3 * u4);
            float3 wan = float3(sin(t + u5 * 40.0) + 0.5 * sin(t * 2.3 + u6 * 17.0),
                                0.5 * sin(t * 1.7 + u4 * 23.0),
                                sin(t * 0.8 + u6 * 31.0)) * (0.6 + 0.9 * u5);
            float3 p = home + wan;
            // Drawn toward its swarm centre, keeping a little of its own place.
            uint k = id % 3;
            float3 sw = _Swarm[k].xyz + (float3(u1, u2, u4) - 0.5) * float3(5.0, 2.5, 5.0) + wan * 0.5;
            p = lerp(p, sw, _Pull * (0.4 + 0.6 * u6));

            // Light: a faint glow, its own blink, and the flash waves passing.
            float glow = 0.035;
            float blink = sin(_T * (1.2 + 1.6 * u4) + u5 * 60.0);
            blink = saturate(blink);
            blink = pow(blink, 24.0);
            float wave = 0.0;
            [unroll] for (int j = 0; j < 4; j++)
            {
                float r = _Wave[j].w * 9.0;
                float d = (length(p - _Wave[j].xyz) - r) / 2.2;
                wave += exp(-d * d) * exp(-_Wave[j].w * 0.35);
            }
            float lum = glow + 0.45 * blink + 0.6 * saturate(wave);
            float3 nat = lerp(float3(0.5, 1.0, 0.08), float3(1.0, 0.62, 0.08), u6 * 0.7);
            float3 pal = TPalette(u6 * 0.6 + _T * 0.01);
            float3 col = lerp(nat, pal * pal * 1.4, _PalMix);
            o.col = col * lum * _Gain * 16.0;

            // A small disc facing the camera, never under a couple of pixels.
            float dist = length(p - _WorldSpaceCameraPos);
            float size = max(0.045 * _Size * (0.7 + 0.6 * u5), dist * 0.0018);
            float3 right = UNITY_MATRIX_V[0].xyz, up = UNITY_MATRIX_V[1].xyz;
            o.pos = TransformWorldToHClip(p + (right * q.x + up * q.y) * size);
            return o;
        }
        ENDHLSL

        Pass
        {
            Tags { "LightMode"="UniversalForward" }
            HLSLPROGRAM
            #pragma vertex vert
            #pragma fragment frag
            #pragma target 4.5
            V vert(uint vid : SV_VertexID) { return Fly(vid); }
            float4 frag(V i) : SV_Target
            {
                float r2 = dot(i.q, i.q);
                clip(1.0 - r2);
                return float4(i.col * (1.0 - 0.5 * r2), 1);
            }
            ENDHLSL
        }
        Pass
        {
            Tags { "LightMode"="DepthOnly" }
            ColorMask R
            HLSLPROGRAM
            #pragma vertex vert
            #pragma fragment frag
            #pragma target 4.5
            V vert(uint vid : SV_VertexID) { return Fly(vid); }
            float4 frag(V i) : SV_Target
            {
                clip(1.0 - dot(i.q, i.q));
                return 0;
            }
            ENDHLSL
        }
    }
}
