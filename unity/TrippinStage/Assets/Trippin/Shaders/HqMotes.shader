// Dust motes for the macro gear shows (unity_speaker): tiny soft specks
// drifting through a box (_BoxC centre, _BoxS size), catching the key light
// (_KeyPos) brighter the closer they float to its beam toward _KeyAim, and
// blown out of up to two ports (_Port[i]: xyz mouth centre, w radius) along
// -z by _Push. No stored state: each mote is a hash of its index.
Shader "Trippin/HqMotes"
{
    Properties
    {
        _Gain ("Gain", Float) = 1
    }
    SubShader
    {
        Tags { "RenderType"="Transparent" "Queue"="Transparent" "RenderPipeline"="UniversalPipeline" }
        Blend One One
        ZWrite Off
        Cull Off
        Pass
        {
            HLSLPROGRAM
            #pragma vertex vert
            #pragma fragment frag
            #pragma target 4.5
            #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/Core.hlsl"

            float _Gain, _T, _Push, _Size;
            float4 _BoxC, _BoxS, _Port[2], _KeyPos, _KeyAim;

            struct V { float4 pos : SV_POSITION; float2 q : TEXCOORD0; float b : TEXCOORD1; };

            uint MHash(uint x) { x ^= x >> 16; x *= 0x7feb352du; x ^= x >> 15; x *= 0x846ca68bu; x ^= x >> 16; return x; }
            float R(uint i, uint s) { return (MHash(i * 7919u + s * 104729u) & 0xffffff) / 16777216.0; }

            V vert(uint vid : SV_VertexID)
            {
                uint id = vid / 6u, k = vid % 6u;
                float2 c = float2(k == 1u || k == 2u || k == 4u ? 1.0 : -1.0, k == 2u || k == 4u || k == 5u ? 1.0 : -1.0);
                float3 h = float3(R(id, 1), R(id, 2), R(id, 3));
                float3 p = _BoxC.xyz + (h - 0.5) * _BoxS.xyz;
                // Slow drift: each mote wanders on its own.
                p += float3(sin(_T * 0.21 + h.x * 40.0), sin(_T * 0.17 + h.y * 37.0) * 0.6 + _T * 0.02 * (h.z - 0.3), sin(_T * 0.13 + h.z * 29.0)) * 0.25;
                // Blown out of the ports on the kicks.
                [unroll] for (int i = 0; i < 2; i++)
                {
                    float3 d = p - _Port[i].xyz;
                    float rad = length(d.xy) / max(_Port[i].w, 1e-3);
                    float near = exp(-rad * rad * 0.6) * exp(-abs(d.z) * 0.25);
                    p.z -= _Push * near * (0.6 + 0.8 * R(id, 7));
                    p.xy += normalize(d.xy + 1e-4) * _Push * near * 0.2;
                }
                // Brighter in the key light's beam.
                float3 axis = normalize(_KeyAim.xyz - _KeyPos.xyz);
                float3 lp = p - _KeyPos.xyz;
                float off = length(lp - axis * dot(lp, axis));
                float b = (0.12 + exp(-off * off * 0.08)) * (0.4 + 0.6 * R(id, 5));
                float size = _Size * (0.5 + R(id, 6));
                float3 right = UNITY_MATRIX_V[0].xyz, up = UNITY_MATRIX_V[1].xyz;
                V o;
                o.pos = TransformWorldToHClip(p + (right * c.x + up * c.y) * size);
                o.q = c; o.b = b;
                return o;
            }

            float4 frag(V i) : SV_Target
            {
                float d = dot(i.q, i.q);
                float a = saturate(1.0 - d) * saturate(1.0 - d);
                return float4(float3(1.0, 0.92, 0.82) * a * i.b * _Gain, 0);
            }
            ENDHLSL
        }
    }
}
