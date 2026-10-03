// The crowd as real people: generated meshes (tools/crowd_meshes.py, Alice
// + Hunyuan3D), drawn instanced with their texture baked into vertex
// colours. Lit like a festival crowd — dark bodies, a stage-coloured rim on
// the silhouette (the camera mostly looks past them at the stage), a
// colour key on whatever faces the stage, and a wash from the rig above.
// Motion: each person bounces on the kick with their own lag (calm-aware)
// and the upper body sways on the flow clock, more as the track builds.
Shader "Trippin/CrowdMesh"
{
    Properties
    {
        _Rim ("Rim light", Float) = 1.4
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
            float _Rim;
            CBUFFER_END

            struct A { float4 pos : POSITION; float3 n : NORMAL; float4 col : COLOR; UNITY_VERTEX_INPUT_INSTANCE_ID };
            struct V { float4 pos : SV_POSITION; float3 n : TEXCOORD0; float3 wp : TEXCOORD1; float4 col : COLOR; float seed : TEXCOORD2; float h : TEXCOORD3; };

            V vert(A i)
            {
                V o;
                UNITY_SETUP_INSTANCE_ID(i);
                float3 origin = TransformObjectToWorld(float3(0, 0, 0));
                float seed = THash(origin.xz * 7.31);
                // Bounce on the kick, each with their own lag and height.
                float ph = frac(_TBeatPhase - seed * 0.25);
                float bounce = exp(-ph * 6.0) * (1.0 - _TCalm) * (0.05 + 0.09 * seed) * _TIntensity;
                // Sway: the upper body leans side to side on the flow clock
                // (smooth — never raw audio), wider with energy.
                float h = i.pos.y; // 0 at the feet, 1 at the head
                float sway = sin(_TFlow * 1.5708 + seed * 20.0) * (0.03 + 0.05 * _TEnergy) * h * h;
                float3 wp = TransformObjectToWorld(i.pos.xyz);
                wp += normalize(TransformObjectToWorldDir(float3(1, 0, 0))) * sway;
                wp.y += bounce;
                o.wp = wp;
                o.pos = TransformWorldToHClip(wp);
                o.n = TransformObjectToWorldNormal(i.n);
                o.col = i.col;
                o.seed = seed;
                o.h = h;
                return o;
            }

            float4 frag(V i) : SV_Target
            {
                float3 n = normalize(i.n);
                float3 v = normalize(GetCameraPositionWS() - i.wp);
                // Baked texture colours are sRGB bytes; light in linear.
                float3 albedo = i.col.rgb * i.col.rgb;
                float3 hue = TPalette(0.35 + i.seed * 0.3);
                // Rim: the silhouette edge catches the stage behind them.
                float rim = pow(1.0 - saturate(dot(n, v)), 3.0) * _Rim * (0.35 + 0.65 * _TEnergy);
                // Key: from the stage (+z), lighting fronts in the palette.
                float key = saturate(dot(n, normalize(float3(0, 0.35, 1)))) * (0.25 + 0.6 * _TLvl.x) * (1.0 - 0.5 * _TCalm);
                // Wash from the rig above, swelling with the mids.
                float top = saturate(n.y) * (0.06 + 0.22 * _TPres.y);
                float3 c = albedo * (0.025 + key * hue * 1.6 + top * TPalette(0.7 + i.seed * 0.2))
                         + hue * rim * 0.5;
                // Feet sink into the dark floor.
                c *= smoothstep(0.0, 0.12, i.h);
                return float4(min(c, 1.0), 1);
            }
            ENDHLSL
        }
    }
}
