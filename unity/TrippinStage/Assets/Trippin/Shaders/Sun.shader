// The stage "sun": a huge disc + concentric ring + slowly turning rays
// behind the set. _Glow (0..1, from StageDirector) blooms it open on a drop
// and lets it settle over the next bars; the rest of the time it's a faint
// presence. Palette-tinted, additive.
Shader "Trippin/Sun"
{
    Properties
    {
        _Glow ("Glow", Float) = 0
        _HueOff ("Palette offset", Float) = 0.1
    }
    SubShader
    {
        Tags { "RenderType"="Transparent" "Queue"="Transparent-20" "RenderPipeline"="UniversalPipeline" }
        Blend One One
        ZWrite Off
        Cull Off
        Pass
        {
            HLSLPROGRAM
            #pragma vertex vert
            #pragma fragment frag
            #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/Core.hlsl"
            #include "TrippinCommon.hlsl"

            CBUFFER_START(UnityPerMaterial)
            float _Glow, _HueOff;
            CBUFFER_END

            struct A { float4 pos : POSITION; float2 uv : TEXCOORD0; };
            struct V { float4 pos : SV_POSITION; float2 uv : TEXCOORD0; };

            V vert(A i) { V o; o.pos = TransformObjectToHClip(i.pos.xyz); o.uv = i.uv; return o; }

            float4 frag(V i) : SV_Target
            {
                float2 p = i.uv * 2 - 1;
                float r = length(p);
                float a = atan2(p.y, p.x + 1e-5);
                float open = 0.55 + 0.45 * _Glow;             // the disc grows as it blooms
                float disc = smoothstep(0.42 * open, 0.38 * open, r);
                float ring = exp(-pow((r - 0.62 * open) * 22.0, 2.0));
                float rays = pow(saturate(cos(a * 18.0 + _TFlow * 0.15)), 8.0) * smoothstep(1.0, 0.45, r) * step(0.4 * open, r);
                float halo = exp(-r * 3.0) * 0.35;
                float3 hot = TPalette(_HueOff);
                float3 cool = TPalette(_HueOff + 0.3);
                float base = 0.08 + 0.92 * _Glow;
                float3 c = hot * disc * (0.6 + 0.4 * _TLvl.x) + cool * ring * 1.4 + hot * rays * 0.8 + hot * halo;
                return float4(c * base * 2.2 * smoothstep(1.0, 0.9, r), 0);
            }
            ENDHLSL
        }
    }
}
