// unity_pixelbars: the face of one LED pixel bar: 24 square pixels up a
// frosted diffuser, each lit from _Px (24 intensities, 6 x float4) in a
// blend of _ColA (low) and _ColB (high) up the bar, with a soft spill
// between the cells and the dark frosted lens showing when off. Opaque and
// depth-writing, so depth of field focuses it.
Shader "Trippin/HqPixelBar"
{
    Properties
    {
        _ColA ("Colour low", Color) = (1, 0.2, 0.6, 1)
        _ColB ("Colour high", Color) = (0.2, 0.6, 1, 1)
        _Gain ("Gain", Float) = 1
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

            float4 _ColA, _ColB;
            float _Gain;
            float4 _Px[6];

            struct A { float4 pos : POSITION; float2 uv : TEXCOORD0; };
            struct V { float4 pos : SV_POSITION; float2 uv : TEXCOORD0; };

            V vert(A i)
            {
                V o;
                o.pos = TransformObjectToHClip(i.pos.xyz);
                o.uv = i.uv;
                return o;
            }

            float Px(int j)
            {
                j = clamp(j, 0, 23);
                float4 v = _Px[j >> 2];
                int k = j & 3;
                return k == 0 ? v.x : k == 1 ? v.y : k == 2 ? v.z : v.w;
            }

            float4 frag(V i) : SV_Target
            {
                float y = i.uv.y * 24.0;
                int j = (int)floor(y);
                float fy = frac(y) - 0.5, fx = i.uv.x - 0.5;
                float v = Px(j);
                // A rounded square cell, with light spilling a little into the neighbours.
                float cell = 1.0 - smoothstep(0.33, 0.45, max(abs(fx), abs(fy)));
                float spill = 0.18 * (Px(j - 1) * saturate(-fy * 2.0) + Px(j + 1) * saturate(fy * 2.0));
                float3 col = lerp(_ColA.rgb, _ColB.rgb, saturate(i.uv.y));
                float3 lit = col * (v * (cell * 3.2 + 0.35) + spill) * _Gain;
                float3 lens = float3(0.012, 0.012, 0.014) * (0.6 + 0.4 * cell);
                return float4(lens + lit, 1);
            }
            ENDHLSL
        }
    }
}
