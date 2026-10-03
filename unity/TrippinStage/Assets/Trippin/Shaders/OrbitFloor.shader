// Foundry floor: a dark pad far below the machine — a warm radial pool of
// light under the core, etched concentric rings and spokes, fading into
// depth haze. _Glow pools with energy and kicks; _Drift slowly rotates the
// etch noise so the floor feels lit, not printed.
Shader "Trippin/OrbitFloor"
{
    Properties
    {
        _Glow ("Pool glow", Float) = 0.3
        _Drift ("Drift", Float) = 0
    }
    SubShader
    {
        Tags { "RenderType"="Opaque" "RenderPipeline"="UniversalPipeline" }
        Pass
        {
            HLSLPROGRAM
            #pragma vertex vert
            #pragma fragment frag
            #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/Core.hlsl"
            #include "TrippinCommon.hlsl"

            CBUFFER_START(UnityPerMaterial)
            float _Glow, _Drift;
            CBUFFER_END

            struct V { float4 pos : SV_POSITION; float3 wp : TEXCOORD0; };

            V vert(float4 v : POSITION)
            {
                V o;
                o.wp = TransformObjectToWorld(v.xyz);
                o.pos = TransformWorldToHClip(o.wp);
                return o;
            }

            float4 frag(V i) : SV_Target
            {
                float2 p = i.wp.xz;
                float d = length(p);
                float3 c = float3(0.012, 0.014, 0.02) * (0.6 + TNoise(p * 0.4) * 0.6);
                // Warm pool of core-light on the pad — tight and dim, or the
                // floor reads as a glowing slab.
                float pool = exp(-d * 0.16);
                c += float3(0.9, 0.32, 0.06) * pool * _Glow * 0.4;
                // Etched concentric rings and spokes — machined pad detail.
                float rings = pow(saturate(sin(d * 1.7)), 90.0);
                float spokes = pow(saturate(sin(atan2(p.y, p.x) * 10.0)), 120.0) * smoothstep(26.0, 12.0, d);
                c += float3(0.5, 0.65, 0.75) * (rings * 0.05 + spokes * 0.02) * (0.4 + _Glow);
                // Depth haze into the void.
                float dist = length(_WorldSpaceCameraPos - i.wp);
                float fog = 1.0 - exp(-dist / 90.0);
                c = lerp(c, float3(0.005, 0.01, 0.02), fog * 0.85);
                return float4(c, 1);
            }
            ENDHLSL
        }
    }
}
