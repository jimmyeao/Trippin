// The coaster deck: the dark track structure between the neon rails (a ribbon
// mesh, uv.x = lap fraction 0..1). Draws the cross-ties as faint lit bands
// and a thin palette stripe on each edge; both breathe with the music but
// never flash. _Len is the track length in metres (tie spacing reads speed).
Shader "Trippin/CoasterDeck"
{
    Properties
    {
        _Len ("Track length (m)", Float) = 1400
        _Gain ("Gain", Float) = 1
    }
    SubShader
    {
        Tags { "RenderType"="Opaque" "Queue"="Geometry" "RenderPipeline"="UniversalPipeline" }
        ZWrite On
        Cull Off
        Pass
        {
            HLSLPROGRAM
            #pragma vertex vert
            #pragma fragment frag
            #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/Core.hlsl"
            #include "TrippinCommon.hlsl"

            CBUFFER_START(UnityPerMaterial)
            float _Len, _Gain;
            CBUFFER_END

            struct A { float4 pos : POSITION; float2 uv : TEXCOORD0; };
            struct V { float4 pos : SV_POSITION; float2 uv : TEXCOORD0; float3 wp : TEXCOORD1; };

            V vert(A i)
            {
                V o;
                o.wp = TransformObjectToWorld(i.pos.xyz);
                o.pos = TransformWorldToHClip(o.wp);
                o.uv = i.uv;
                return o;
            }

            float4 frag(V i) : SV_Target
            {
                float3 col = float3(0.012, 0.012, 0.02);
                // Ties: a lit strip every ~1.2 m of track.
                float tie = smoothstep(0.06, 0.0, abs(frac(i.uv.x * _Len / 1.2) - 0.5) * 1.2);
                col += float3(0.25, 0.3, 0.4) * tie * 0.25 * (0.5 + 0.5 * TBeatPulse(6.0));
                // Faint neon edge stripes (just inside the rails).
                float edge = smoothstep(0.04, 0.0, min(i.uv.y, 1.0 - i.uv.y));
                col += TPalette(0.55) * edge * 0.12 * (0.5 + _TEnergy);
                // Smog haze with distance.
                float dist = length(i.wp - GetCameraPositionWS());
                float fog = 1.0 - exp(-dist / 420.0);
                col = lerp(col, float3(0.12, 0.035, 0.1), saturate(fog));
                return float4(col * _Gain, 1.0);
            }
            ENDHLSL
        }
    }
}
