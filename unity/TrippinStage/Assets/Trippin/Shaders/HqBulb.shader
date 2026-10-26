// unity_bokeh's bulbs: strings of small glowing glass bulbs, all in one
// mesh (FairyLightsShow.cs builds it: each vertex carries its bulb's index
// in uv.x and its strand's sway phase in uv.y). Opaque and depth-writing,
// so the show's depth of field blurs each bulb by its true distance into a
// bokeh disc. Colour and brightness per bulb come from _BulbCol; the
// strands sway gently (_Sway, _T).
Shader "Trippin/HqBulb"
{
    Properties
    {
        _Gain ("Gain", Float) = 1
        _Sway ("Sway", Float) = 1
        _T ("Time", Float) = 0
    }
    SubShader
    {
        Tags { "RenderType"="Opaque" "Queue"="Geometry" "RenderPipeline"="UniversalPipeline" }
        Pass
        {
            HLSLPROGRAM
            #pragma vertex vert
            #pragma fragment frag
            #pragma target 4.5
            #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/Core.hlsl"

            CBUFFER_START(UnityPerMaterial)
            float _Gain, _Sway, _T;
            CBUFFER_END
            float4 _BulbCol[512];

            struct A { float4 pos : POSITION; float3 n : NORMAL; float2 uv : TEXCOORD0; };
            struct V { float4 pos : SV_POSITION; float3 col : TEXCOORD0; float3 n : TEXCOORD1; float3 v : TEXCOORD2; };

            V vert(A i)
            {
                V o;
                float3 p = i.pos.xyz;
                // Strands sway in the breeze: more toward the middle of the span.
                float ph = i.uv.y;
                p.z += sin(_T * 0.7 + ph) * 0.25 * _Sway;
                p.y += sin(_T * 0.9 + ph * 1.7) * 0.06 * _Sway;
                float3 wp = TransformObjectToWorld(p);
                o.pos = TransformWorldToHClip(wp);
                o.col = _BulbCol[(int)i.uv.x].rgb;
                o.n = TransformObjectToWorldNormal(i.n);
                o.v = normalize(_WorldSpaceCameraPos - wp);
                return o;
            }

            float4 frag(V i) : SV_Target
            {
                // A glass bulb: a hot core facing you, softer toward the rim.
                float nv = saturate(dot(normalize(i.n), i.v));
                float3 c = i.col * (0.35 + 0.65 * nv * nv) + i.col * pow(nv, 12.0) * 0.8;
                return float4(c * _Gain, 1);
            }
            ENDHLSL
        }
    }
}
