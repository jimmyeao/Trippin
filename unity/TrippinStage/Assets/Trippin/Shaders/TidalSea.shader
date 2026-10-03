Shader "Trippin/TidalSea"
{
    Properties { }
    SubShader
    {
        Tags { "RenderType"="Transparent" "Queue"="Transparent-20" "RenderPipeline"="UniversalPipeline" }
        Cull Off
        Blend SrcAlpha OneMinusSrcAlpha
        ZWrite Off
        Pass
        {
            HLSLPROGRAM
            #pragma vertex vert
            #pragma fragment frag
            #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/Core.hlsl"
            #include "TrippinCommon.hlsl"

            float _Current, _Energy, _Kick;
            struct A { float4 pos : POSITION; };
            struct V { float4 pos : SV_POSITION; float3 wp : TEXCOORD0; };
            V vert(A i)
            {
                V o;
                float3 p = TransformObjectToWorld(i.pos.xyz);
                p.y += (sin(p.z * 0.22 - _Current * 1.4 + p.x * 0.14) + sin(p.x * 0.29 + p.z * 0.11 + _Current)) * (0.06 + 0.14 * _Energy + 0.35 * _Kick);
                o.wp = p;
                o.pos = TransformWorldToHClip(p);
                return o;
            }
            float4 frag(V i) : SV_Target
            {
                float3 n = normalize(cross(ddx(i.wp), ddy(i.wp)));
                if (n.y < 0.0) n = -n;
                float3 v = normalize(GetCameraPositionWS() - i.wp);
                float fres = pow(1.0 - saturate(dot(n, v)), 3.0);
                float x = i.wp.x, z = i.wp.z;
                float nearestRow = clamp(round(z / 24.0), 0.0, 3.0);
                float bank = 12.0 + fmod(nearestRow, 2.0) * 2.5;
                float2 d = float2(abs(x) - bank, z - nearestRow * 24.0);
                float baseGlow = exp(-dot(d, d) * 0.014);
                float current = z * 0.2 - _Current * 2.6;
                float waves = sin(current + x * 0.9 + sin(z * 0.17)) * sin(x * 0.38 - current * 0.45);
                float streaks = pow(saturate(waves), 7.0) * (0.14 + 0.6 * fres) * (1.0 + 1.1 * _Kick);
                float3 c = float3(0.003, 0.019, 0.029) + float3(0.015, 0.1, 0.14) * fres;
                c += lerp(float3(0.025, 0.23, 0.29), TPalette(0.52), 0.28) * baseGlow * (0.36 + streaks);
                c += float3(0.05, 0.23, 0.32) * streaks * 0.2;
                float aisle = exp(-x * x * 0.03) * smoothstep(-50.0, -10.0, z) * (1.0 - smoothstep(130.0, 170.0, z));
                float2 q = float2(x * 0.22 + sin(z * 0.17) * 0.6, z * 0.13 - _Current * 0.09);
                float fractured = smoothstep(0.15, 0.4, TNoise(q) * TNoise(q * 2.1 + 4.3));
                c += float3(0.025, 0.2, 0.27) * aisle * (0.1 + fractured * 0.6) * (0.4 + 0.6 * fres);
                float fog = 1.0 - exp(-length(i.wp - GetCameraPositionWS()) / 115.0);
                c = lerp(c, float3(0.015, 0.055, 0.069), fog * 0.45);
                return float4(c, saturate(0.42 + fres * 0.3 + fog * 0.12));
            }
            ENDHLSL
        }
    }
}
