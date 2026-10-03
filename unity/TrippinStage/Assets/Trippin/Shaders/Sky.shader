// Night sky on a big inverted sphere: dark zenith, a palette-tinted glow on
// the horizon (the city's light in the fog), sparse stars above it.
Shader "Trippin/Sky"
{
    Properties { }
    SubShader
    {
        Tags { "RenderType"="Opaque" "Queue"="Background" "RenderPipeline"="UniversalPipeline" }
        Cull Front
        ZWrite Off
        Pass
        {
            HLSLPROGRAM
            #pragma vertex vert
            #pragma fragment frag
            #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/Core.hlsl"
            #include "TrippinCommon.hlsl"

            float _Glow;

            struct A { float4 pos : POSITION; };
            struct V { float4 pos : SV_POSITION; float3 d : TEXCOORD0; };
            V vert(A i) { V o; o.pos = TransformObjectToHClip(i.pos.xyz); o.d = normalize(i.pos.xyz); return o; }

            float4 frag(V i) : SV_Target
            {
                float3 d = normalize(i.d);
                float h = d.y;
                float3 zen = float3(0.004, 0.005, 0.012);
                float3 hor = TPalette(0.45) * 0.10 + float3(0.03, 0.025, 0.03);
                float3 c = lerp(hor, zen, smoothstep(-0.02, 0.45, h));
                c += TPalette(0.5) * exp(-abs(h) * 14.0) * 0.12 * (0.6 + _Glow);
                // Stars: sparse cells above the haze.
                float2 g = float2(atan2(d.z, d.x) * 120.0, h * 240.0);
                float2 cell = floor(g);
                uint2 q = (uint2)(int2)(cell + 32768.0);
                q = q * uint2(1597334673u, 3812015801u);
                float r = ((q.x ^ q.y) * 1597334673u) * (1.0 / 4294967296.0);
                float star = step(0.996, r) * smoothstep(0.35, 0.0, length(frac(g) - 0.5)) * smoothstep(0.12, 0.5, h);
                c += star * 0.8;
                return float4(c, 1);
            }
            ENDHLSL
        }
    }
}
