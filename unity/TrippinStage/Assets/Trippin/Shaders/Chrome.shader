// Polished chrome / crystal facets, instanced. Reflects a procedural studio
// (palette bands, strip lights orbiting on the energy clock, a hot key
// light above), with a fresnel rim and distance fog — the faceted-chrome
// look of festival screen content without reflection probes.
Shader "Trippin/Chrome"
{
    Properties
    {
        _Fog ("Fog density", Float) = 0.012
        _Tint ("Tint amount", Float) = 0.6
    }
    SubShader
    {
        Tags { "RenderType"="Opaque" "Queue"="Geometry" "RenderPipeline"="UniversalPipeline" }
        Pass
        {
            HLSLPROGRAM
            #pragma vertex vert
            #pragma fragment frag
            #pragma multi_compile_instancing
            #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/Core.hlsl"
            #include "TrippinCommon.hlsl"

            CBUFFER_START(UnityPerMaterial)
            float _Fog, _Tint;
            CBUFFER_END

            struct A { float4 pos : POSITION; float3 n : NORMAL; UNITY_VERTEX_INPUT_INSTANCE_ID };
            struct V { float4 pos : SV_POSITION; float3 wp : TEXCOORD0; float3 n : TEXCOORD1; float seed : TEXCOORD2; };

            V vert(A i)
            {
                V o;
                UNITY_SETUP_INSTANCE_ID(i);
                o.wp = TransformObjectToWorld(i.pos.xyz);
                o.pos = TransformWorldToHClip(o.wp);
                o.n = TransformObjectToWorldNormal(i.n);
                float3 origin = TransformObjectToWorld(float3(0, 0, 0));
                o.seed = THash(origin.xz * 1.3 + origin.y * 7.1);
                return o;
            }

            // The studio the chrome reflects.
            float3 env(float3 r, float seed)
            {
                float az = atan2(r.x, r.z) / 6.2831853;
                float3 c = TPalette(r.y * 0.25 + 0.5 + seed * 0.2) * (0.12 + 0.25 * saturate(r.y + 0.3));
                // Two rings of strip lights orbiting in opposite directions.
                float s1 = pow(saturate(1.0 - abs(frac(az * 6.0 + _TClock.x * 0.04) - 0.5) * 9.0), 2.0)
                         * smoothstep(-0.3, 0.2, r.y) * smoothstep(0.95, 0.4, r.y);
                float s2 = pow(saturate(1.0 - abs(frac(az * 9.0 - _TClock.y * 0.03 + 0.25) - 0.5) * 12.0), 2.0)
                         * smoothstep(-0.8, -0.3, r.y) * smoothstep(0.1, -0.2, r.y);
                c += TPalette(az + _TFlow * 0.01) * s1 * (4.0 + 3.0 * _TLvl.y);
                c += TPalette(az + 0.5 - _TFlow * 0.01) * s2 * (2.5 + 2.0 * _TLvl.z);
                // Hot key light overhead, and a palette floor bounce.
                c += float3(1.0, 0.97, 0.92) * pow(saturate(r.y), 8.0) * 5.0;
                c += TPalette(0.15) * pow(saturate(-r.y), 2.0) * 0.6;
                return c;
            }

            float4 frag(V i) : SV_Target
            {
                float3 n = normalize(i.n);
                float3 v = normalize(i.wp - _WorldSpaceCameraPos);
                float3 r = reflect(v, n);
                float fres = pow(1.0 - saturate(dot(-v, n)), 3.0);
                float3 tint = lerp(float3(1, 1, 1), TPalette(i.seed * 0.3 + 0.1) * 1.6, _Tint);
                float3 c = env(r, i.seed) * tint * (0.6 + 0.8 * fres);
                c += TPalette(i.seed + 0.5) * fres * 1.2 * (0.5 + _TEnergy);
                float d = length(i.wp - _WorldSpaceCameraPos);
                float fog = 1.0 - exp(-d * _Fog);
                c = lerp(c, TPalette(0.05) * 0.03, fog);
                return float4(c, 1);
            }
            ENDHLSL
        }
    }
}
