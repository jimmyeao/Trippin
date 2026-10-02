// Laser / light-shaft beam: a quad running along the object's local +Y (0..1,
// scale Y = length), billboarded around that axis so it always faces the
// camera. Thin saturated core + soft glow, made patchy along its length by
// world-space smoke noise — beams cutting through fog-machine haze, not neon
// tubes. _Width/_Spread make the wide, diverging light shafts.
Shader "Trippin/Beam"
{
    Properties
    {
        _Color ("Color", Color) = (1,0,0,1)
        _Intensity ("Intensity", Float) = 4
        _Width ("Width at source (m)", Float) = 0.03
        _Spread ("Width growth per metre", Float) = 0.002
        _Core ("Core sharpness", Float) = 40
        _Smoke ("Smoke patchiness 0..1", Float) = 0.7
        _Fade ("Length fade start 0..1", Float) = 0.55
        _Hot ("White-hot core", Float) = 0.6
    }
    SubShader
    {
        Tags { "RenderType"="Transparent" "Queue"="Transparent" "RenderPipeline"="UniversalPipeline" }
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
            float4 _Color;
            float _Intensity, _Width, _Spread, _Core, _Smoke, _Fade, _Hot;
            CBUFFER_END

            struct A { float4 pos : POSITION; float2 uv : TEXCOORD0; };
            struct V { float4 pos : SV_POSITION; float2 uv : TEXCOORD0; float3 wp : TEXCOORD1; float len : TEXCOORD2; };

            V vert(A i)
            {
                V o;
                float3 s = TransformObjectToWorld(float3(0, 0, 0));
                float3 e = TransformObjectToWorld(float3(0, 1, 0));
                float3 d = e - s;
                float len = length(d);
                d /= max(len, 1e-4);
                float y = i.uv.y;
                float3 p = s + d * (y * len);
                float3 view = normalize(_WorldSpaceCameraPos - p);
                float3 side = normalize(cross(d, view) + 1e-5);
                float w = _Width + _Spread * y * len;
                p += side * (i.uv.x * 2 - 1) * w * 3.0; // quad 3x wider than the core for glow
                o.pos = TransformWorldToHClip(p);
                o.uv = float2(i.uv.x * 2 - 1, y);
                o.wp = p;
                o.len = len;
                return o;
            }

            float4 frag(V i) : SV_Target
            {
                float x = i.uv.x * 3.0; // in core widths
                float core = exp(-x * x * _Core * 0.05);
                float glow = exp(-x * x * 0.9) * 0.35 * saturate(_Hot * 2.0 + 0.3);
                // Haze: the beam is only visible where smoke hangs.
                float3 q = i.wp * 0.22 + float3(0, -_TFlow * 0.08, _TFlow * 0.05);
                float smoke = lerp(1.0, smoothstep(0.25, 0.85, TFbm3(q)) * 1.6, _Smoke);
                float along = 1.0 - smoothstep(_Fade, 1.0, i.uv.y);
                float near = smoothstep(0.0, 0.02, i.uv.y);
                // Lasers get a white-hot centre; shafts stay pure colour.
                float3 c = _Color.rgb * (core * 3.0 + glow) + core * _Hot;
                // Shafts brighten toward the fixture like a real cone of light.
                c *= lerp(1.0, 1.6 - i.uv.y, saturate(1.0 - _Hot * 2.0));
                return float4(c * smoke * along * near * _Intensity, 0);
            }
            ENDHLSL
        }
    }
}
