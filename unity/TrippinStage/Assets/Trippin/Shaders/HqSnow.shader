// unity_winter's snow and lights, worked out from their index in the vertex
// shader (no stored state), drawn as small opaque depth-writing discs so the
// show's depth of field blurs each by its true distance into bokeh.
// One draw: the first _Flakes are snow, the rest bulbs.
//  Snow:    snowflakes falling through a deep box in front of the village,
//           wrapping at the bottom and the sides, drifting on the wind
//           clock and swirled round the gusts (_Gust[2]: centre, strength).
//  Bulbs:   strings of coloured bulbs sagging across the foreground
//           (_Strands of them), a chase running along them (_Chase: head
//           position 0..1 per strand, x; strength, y).
Shader "Trippin/HqSnow"
{
    Properties
    {
        _Gain ("Gain", Float) = 1
        _Size ("Size", Float) = 1
    }
    SubShader
    {
        Tags { "RenderType"="Opaque" "Queue"="Geometry" "RenderPipeline"="UniversalPipeline" }
        Cull Off

        HLSLINCLUDE
        #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/Core.hlsl"
        #include "TrippinCommon.hlsl"

        float _Gain, _Size, _Flakes, _Bulbs, _T, _Wind, _Hue, _Warmth;
        float4 _Gust[2];
        float4 _Chase;

        struct V { float4 pos : SV_POSITION; float2 q : TEXCOORD0; float3 col : TEXCOORD1; };

        uint SHash(uint x)
        {
            x ^= x >> 16; x *= 0x7feb352du; x ^= x >> 15; x *= 0x846ca68bu; x ^= x >> 16;
            return x;
        }
        float R(uint i, uint salt) { return (SHash(i * 9781u + salt * 6271u) & 0xffffff) / 16777216.0; }

        V Flake(uint vid)
        {
            V o;
            uint id = vid / 6, c = vid % 6;
            bool snow = id < (uint)_Flakes;
            if (!snow) id -= (uint)_Flakes;
            float2 corner[6] = { float2(-1,-1), float2(1,-1), float2(-1,1), float2(-1,1), float2(1,-1), float2(1,1) };
            float2 q = corner[c];
            o.q = q;
            float u1 = R(id, 1), u2 = R(id, 2), u3 = R(id, 3), u4 = R(id, 4), u5 = R(id, 5), u6 = R(id, 6);
            float3 p, col;
            float size;
            if (snow)
            {
                // Snow: deep box, wider further away; falls, wraps, drifts.
                float z = 1.5 + 60.0 * pow(u3, 1.4);
                float W = 4.0 + 0.75 * z;
                const float H = 20.0;
                float v = 0.7 + 0.7 * u4;
                float y = H - frac(u2 + _T * v / H) * H - 1.0;
                float x = (u1 - 0.5) * 2.0 * W + _Wind * (0.6 + 0.8 * u5) + sin(_T * 0.9 + u6 * 30.0) * 0.35;
                x = (frac(x / (2.0 * W) + 0.5) - 0.5) * 2.0 * W;
                p = float3(x, y, z);
                // Gusts swirl the snow round them.
                [unroll] for (int g = 0; g < 2; g++)
                {
                    float3 d = p - _Gust[g].xyz;
                    float s = _Gust[g].w * exp(-dot(d, d) / 30.0);
                    float a = s * 2.2;
                    float2 r = float2(d.x * cos(a) - d.y * sin(a), d.x * sin(a) + d.y * cos(a));
                    p = _Gust[g].xyz + float3(r.x, r.y, d.z);
                }
                // Moonlight blue-white, warmer low down near the village lights.
                col = lerp(float3(0.75, 0.85, 1.05), float3(1.1, 0.85, 0.6), saturate((4.0 - p.y) / 6.0) * _Warmth) * (0.7 + 0.5 * u5);
                size = 0.022 * _Size * (0.6 + 0.8 * u6);
            }
            else
            {
                // Bulbs: strings sagging across the foreground.
                uint per = 44;
                uint strand = id / per, b = id % per;
                float s = (b + 0.5) / per;
                float zs = 3.2 + strand * 1.7;
                float span = 3.5 + zs * 0.9;
                float x = (s - 0.5) * 2.0 * span;
                float y = 4.0 + strand * 0.75 - (0.95 - strand * 0.2) * sin(s * 3.14159) + 0.06 * sin(_T * 0.8 + strand * 2.0 + s * 6.0);
                p = float3(x, y, zs);
                float hue = _Hue + strand * 0.21 + floor(b % 4) * 0.25;
                float3 pal = TPalette(hue);
                pal *= pal;
                // A chase runs along each string.
                float d = frac(s - _Chase.x - strand * 0.17);
                d = min(d, 1.0 - d) / 0.06;
                float lit = 0.9 + 0.15 * sin(_T * 3.0 + b * 1.7) + _Chase.y * 2.5 * exp(-d * d);
                col = pal * 1.6 * lit * 9.0;
                size = 0.06;
            }
            o.col = col * _Gain * (snow ? 1.0 : _Bulbs);
            float dist = length(p - _WorldSpaceCameraPos);
            size = max(size, dist * 0.0012);
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
            V vert(uint vid : SV_VertexID) { return Flake(vid); }
            float4 frag(V i) : SV_Target
            {
                float r2 = dot(i.q, i.q);
                clip(1.0 - r2);
                return float4(i.col * (1.0 - 0.4 * r2), 1);
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
            V vert(uint vid : SV_VertexID) { return Flake(vid); }
            float4 frag(V i) : SV_Target
            {
                clip(1.0 - dot(i.q, i.q));
                return 0;
            }
            ENDHLSL
        }
    }
}
