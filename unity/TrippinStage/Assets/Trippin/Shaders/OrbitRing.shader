// Gimbal ring / machined metal: a dark gunmetal torus with emissive segment
// ticks scrolling along its outer rim on _Scroll. The metal reads through
// fresnel rim light; _Tint picks the emissive hue family, _Glow the level.
// Also used for the gantry housing and weld pods (cubes shade the same).
Shader "Trippin/OrbitRing"
{
    Properties
    {
        _Tint ("Emissive hue", Float) = 0.55
        _Scroll ("Tick scroll", Float) = 0
        _Glow ("Emissive level", Float) = 0.2
        _Segs ("Dash count", Float) = 7
    }
    SubShader
    {
        Tags { "RenderType"="Opaque" "RenderPipeline"="UniversalPipeline" }
        Cull Off // thin tubes at grazing angles: each twisted quad's far
                 // triangle folds past the silhouette and would be culled,
                 // leaving a woven lattice of holes
        Pass
        {
            HLSLPROGRAM
            #pragma vertex vert
            #pragma fragment frag
            #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/Core.hlsl"
            #include "TrippinCommon.hlsl"

            CBUFFER_START(UnityPerMaterial)
            float _Tint, _Scroll, _Glow, _Segs;
            CBUFFER_END

            struct V { float4 pos : SV_POSITION; float2 uv : TEXCOORD0; float3 n : TEXCOORD1; float3 wp : TEXCOORD2; };

            V vert(float4 v : POSITION, float3 n : NORMAL, float2 uv : TEXCOORD0)
            {
                V o;
                o.wp = TransformObjectToWorld(v.xyz);
                o.pos = TransformWorldToHClip(o.wp);
                o.n = TransformObjectToWorldNormal(n);
                o.uv = uv;
                return o;
            }

            float4 frag(V i) : SV_Target
            {
                float3 n = normalize(i.n);
                float3 view = normalize(_WorldSpaceCameraPos - i.wp);
                if (dot(n, view) < 0.0) n = -n; // two-sided: shade tube interiors sanely
                float fres = pow(1.0 - saturate(dot(n, view)), 2.2);
                float3 tint = TPalette(_Tint);
                // Machined gunmetal: dark body, cool sky sheen wrapping the
                // tube, warm bounce from the core above the floor line.
                float3 c = float3(0.016, 0.02, 0.03)
                         + fres * float3(0.1, 0.15, 0.22) * 0.9
                         + float3(0.25, 0.1, 0.03) * saturate(-i.wp.y + 2.0) * 0.006;
                // Brushed-metal sheen: a soft bright band along the top of the
                // tube gives it a milled look instead of bare facets.
                float top = pow(saturate(1.0 - abs(i.uv.y * 2.0 - 0.62)), 8.0);
                c += float3(0.12, 0.16, 0.2) * top * 0.3;
                // One thin glowing seam on the outer rim + a slow travelling
                // pulse — never a wrap-around pattern (that reads as wicker).
                float rim = smoothstep(0.88, 0.97, abs(i.uv.y * 2.0 - 1.0));
                float runner = pow(saturate(1.0 - abs(frac(i.uv.x - _Scroll) * 2.0 - 1.0)), 10.0);
                c += tint * rim * (0.3 + runner * 1.4) * _Glow;
                return float4(c, 1);
            }
            ENDHLSL
        }
    }
}
