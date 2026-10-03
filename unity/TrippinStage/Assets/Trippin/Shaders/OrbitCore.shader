// The foundry core: a unit sphere displaced in the vertex stage — _Amp
// noise ripples it with bass, _Kick compresses it. The fragment stage
// shades with flat facet normals (ddx/ddy) so the tiles read as a mirror
// ball rather than a dented skin. Dark iron body, hot emissive seams and
// a heated fresnel rim — the metal glows from within.
Shader "Trippin/OrbitCore"
{
    Properties
    {
        _Morph ("Shape (sx,sy,sz,waist)", Vector) = (1,1,1,0)
        _Bass ("Bass", Float) = 0
        _Kick ("Kick", Float) = 0
        _Heat ("Heat", Float) = 0.5
        _Amp ("Surface roughness", Float) = 0.1
        _NoiseT ("Noise clock", Float) = 0
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
            float4 _Morph;
            float _Bass, _Kick, _Heat, _Amp, _NoiseT;
            CBUFFER_END

            // Displaced object-space position of a unit-sphere direction.
            float3 Disp(float3 n)
            {
                float3 p = n * _Morph.xyz;
                // Waist pinch (or equatorial belt) along the deformed Y.
                p.xz *= 1.0 - _Morph.w * 0.35f * (1.0 - n.y * n.y);
                // Molten surface noise, advected on the flow clock — low
                // frequency or the surface spikes into an urchin.
                float3 q = n * 1.35;
                float turb = TFbm3(q + float3(_NoiseT * 0.6, _NoiseT * 0.4, -_NoiseT * 0.5)) - 0.45;
                // Damp the negative side: dips in the metal read as dents,
                // raised ripples read as molten texture.
                turb = turb < 0.0 ? turb * 0.25 : turb;
                p += n * turb * _Amp * (0.6 + _Bass * 1.6);
                // Bass swells the mass, the kick compresses it.
                p *= 1.0 + _Bass * 0.16 - _Kick * 0.12;
                return p;
            }

            struct V { float4 pos : SV_POSITION; float3 wp : TEXCOORD0; };

            V vert(float4 v : POSITION)
            {
                V o;
                float3 p = Disp(normalize(v.xyz));
                o.wp = TransformObjectToWorld(p);
                o.pos = TransformWorldToHClip(o.wp);
                return o;
            }

            float4 frag(V i) : SV_Target
            {
                // Flat facet normal — each mesh tile shades uniformly, so the
                // surface reads as mirror-ball facets instead of dents.
                float3 n = normalize(cross(ddx(i.wp), ddy(i.wp)));
                float3 view = normalize(_WorldSpaceCameraPos - i.wp);
                if (dot(n, view) < 0.0) n = -n;
                float fres = pow(1.0 - saturate(dot(n, view)), 2.6);
                // Hot cracks: noise veins glowing through the dark skin.
                float vein = TFbm3(i.wp * 0.55 + _NoiseT * 0.3);
                // Molten seams only in patches — veins everywhere read as
                // gold speckle, not iron.
                float patch = smoothstep(0.42, 0.6, TFbm3(i.wp * 0.22 + 7.3));
                float cracks = smoothstep(0.56, 0.7, vein) * smoothstep(0.56, 0.7, vein) * 2.8 * patch;
                float3 iron = float3(0.055, 0.05, 0.075) + fres * float3(0.14, 0.1, 0.11);
                float3 hot = float3(1.0, 0.3, 0.05);
                float3 c = iron
                         + hot * cracks * (0.7 + _Heat * 2.6)
                         + hot * fres * _Heat * 0.35
                         + TPalette(0.6) * fres * 0.32; // cool sky reflection on the rim
                return float4(c, 1);
            }
            ENDHLSL
        }
    }
}
