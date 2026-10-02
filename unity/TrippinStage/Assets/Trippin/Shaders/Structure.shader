// Stage steel: near-black metal lit by the stage's own glow (palette ambient
// from below/behind), with emissive edge strips along the uv border. The
// geometry is static — only the light on it moves (house rule: architecture
// never changes shape).
Shader "Trippin/Structure"
{
    Properties
    {
        _Base ("Base", Color) = (0.02,0.02,0.025,1)
        _Strip ("Edge strip width (uv)", Float) = 0.04
        _StripGain ("Strip brightness", Float) = 2
        _HueOff ("Palette offset", Float) = 0
        _Floor ("Is floor (reflective sheen)", Float) = 0
    }
    SubShader
    {
        Tags { "RenderType"="Opaque" "Queue"="Geometry" "RenderPipeline"="UniversalPipeline" }
        Pass
        {
            HLSLPROGRAM
            #pragma vertex vert
            #pragma fragment frag
            #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/Core.hlsl"
            #include "TrippinCommon.hlsl"

            CBUFFER_START(UnityPerMaterial)
            float4 _Base;
            float _Strip, _StripGain, _HueOff, _Floor;
            CBUFFER_END

            struct A { float4 pos : POSITION; float3 n : NORMAL; float2 uv : TEXCOORD0; };
            struct V { float4 pos : SV_POSITION; float2 uv : TEXCOORD0; float3 wp : TEXCOORD1; float3 n : TEXCOORD2; };

            V vert(A i)
            {
                V o;
                o.wp = TransformObjectToWorld(i.pos.xyz);
                o.pos = TransformWorldToHClip(o.wp);
                o.n = TransformObjectToWorldNormal(i.n);
                o.uv = i.uv;
                return o;
            }

            float4 frag(V i) : SV_Target
            {
                float3 glow = TPalette(_HueOff + 0.35) * (0.15 + 0.35 * _TEnergy);
                // Light from the LED walls behind/above: faces toward the
                // crowd (−z) catch less of it than tops and sides.
                float facing = saturate(0.4 + 0.6 * dot(normalize(i.n), float3(0, 0.6, 0.8)));
                float3 c = _Base.rgb + glow * 0.06 * facing;
                float2 e = min(i.uv, 1.0 - i.uv);
                float strip = 1.0 - smoothstep(_Strip * 0.6, _Strip, min(e.x, e.y));
                float chase = 0.55 + 0.45 * sin(i.wp.y * 0.8 - _TClock.z * 1.5 + i.wp.x * 0.15);
                c += TPalette(_HueOff + i.wp.y * 0.02) * strip * _StripGain * chase * (0.4 + 0.8 * _TPres.z);
                if (_Floor > 0.5) {
                    // Wet-floor sheen: the walls' colour smeared toward the stage.
                    float sheen = exp(-abs(i.wp.x) * 0.04) * smoothstep(-40.0, 8.0, i.wp.z);
                    c += TPalette(_HueOff + 0.3 + i.wp.x * 0.01) * sheen * 0.08 * (0.5 + _TEnergy);
                }
                return float4(c, 1);
            }
            ENDHLSL
        }
    }
}
