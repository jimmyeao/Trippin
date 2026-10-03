Shader "Trippin/TidalSail"
{
    Properties { }
    SubShader
    {
        Tags { "RenderType"="Transparent" "Queue"="Transparent" "RenderPipeline"="UniversalPipeline" }
        Cull Off
        Blend SrcAlpha OneMinusSrcAlpha
        ZWrite Off
        Pass
        {
            HLSLPROGRAM
            #pragma vertex vert
            #pragma fragment frag
            #pragma target 4.5
            #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/Core.hlsl"
            #include "TrippinCommon.hlsl"

            float _Height, _Width, _Side, _Phase, _Hue, _Bass, _Mids, _Highs, _Current, _Swing, _Kick;
            struct A { float4 pos : POSITION; float2 uv : TEXCOORD0; };
            struct V { float4 pos : SV_POSITION; float3 wp : TEXCOORD0; float2 uv : TEXCOORD1; float fold : TEXCOORD2; float3 normal : TEXCOORD3; };
            float4 shape(float2 uv)
            {
                float u = uv.x * 2.0 - 1.0, v = uv.y;
                float breadth = (0.35 + 0.85 * sin(v * 3.141593)) * pow(1.0 - v, 0.45) + 0.015;
                float fold = sin(v * 14.0 - _Current * 1.55 + u * 2.6 + _Phase) * sin(v * 3.141593) * (0.35 + 1.2 * _Mids + 1.8 * _Kick);
                float reach = 9.5 * v * v + _Swing * v * v * 1.7;
                float3 p = float3(_Side * (u * _Width * breadth * (0.5 + 0.13 * _Bass + 0.22 * _Kick) - reach) + fold * 0.45,
                                  v * _Height + fold * 0.13,
                                  sin(v * 5.0 + _Current * 0.36 + _Phase) * v * v * 3.2
                                  + (u * u - 0.25) * (3.8 + 1.2 * _Bass + 4.5 * _Kick) + fold * 1.4 + u * 1.3);
                return float4(p, fold);
            }
            V vert(A i)
            {
                V o;
                float4 shaped = shape(i.uv);
                float3 du = shape(float2(saturate(i.uv.x + 0.006), i.uv.y)).xyz - shape(float2(saturate(i.uv.x - 0.006), i.uv.y)).xyz;
                float3 dv = shape(float2(i.uv.x, saturate(i.uv.y + 0.006))).xyz - shape(float2(i.uv.x, saturate(i.uv.y - 0.006))).xyz;
                o.wp = TransformObjectToWorld(shaped.xyz);
                o.pos = TransformWorldToHClip(o.wp);
                o.uv = i.uv;
                o.fold = shaped.w;
                o.normal = TransformObjectToWorldNormal(normalize(cross(du, dv)));
                return o;
            }

            float4 frag(V i) : SV_Target
            {
                float3 n = normalize(i.normal);
                float3 view = normalize(GetCameraPositionWS() - i.wp);
                float nv = abs(dot(n, view));
                float rim = pow(1.0 - nv, 2.5);
                float edge = pow(abs(i.uv.x * 2.0 - 1.0), 12.0);
                float veins = pow(saturate(cos(i.uv.x * 40.0 + i.uv.y * 6.0 + i.fold * 2.0)), 28.0);
                float glint = pow(saturate(dot(reflect(-view, n), normalize(float3(-0.42, 0.75, -0.5)))), 25.0);
                float film = sin(i.uv.y * 24.0 + i.uv.x * 6.0 - _Current * 2.5 + i.fold * 1.7);
                float3 ink = lerp(float3(0.004, 0.026, 0.05), TPalette(_Hue), 0.16);
                float3 sheen = lerp(float3(0.02, 0.11, 0.19), TPalette(_Hue + 0.18), 0.15);
                float3 c = ink * (0.7 + 0.25 * film) + sheen * (0.22 + rim * 0.95 + veins * 0.16);
                c += float3(0.23, 0.86, 1.0) * (edge * (0.4 + 0.18 * _Highs) + glint * 0.85);
                c += sheen * rim * rim * 0.65;
                float alpha = (0.43 + rim * 0.27 + edge * 0.13) * smoothstep(0.0, 0.055, i.uv.y) * smoothstep(1.0, 0.94, i.uv.y);
                return float4(c, saturate(alpha));
            }
            ENDHLSL
        }
    }
}
