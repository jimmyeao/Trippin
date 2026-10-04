// A wire tunnel: a grid mesh (uv.x = angle, uv.y = length) wrapped into a
// tube along +z, drawn as ring and spoke lines. The camera sits inside at the
// origin looking down +z; _Scroll slides the pattern past it (flight without
// moving geometry). The tube snakes (_BendAmp) and its radius wobbles (_Wob)
// along its length. Additive. The show drives _Wob (eased bass), _Scroll and
// _Phase (the smooth energy clock) and _Intensity; nothing reads raw audio.
Shader "Trippin/WireTube"
{
    Properties
    {
        _Radius ("Radius (m)", Float) = 9
        _Length ("Length (m)", Float) = 240
        _Scroll ("Pattern scroll (m)", Float) = 0
        _Phase ("Wave phase", Float) = 0
        _Wob ("Radius wobble", Float) = 0.2
        _BendAmp ("Bend (m)", Float) = 3
        _BendPhase ("Bend phase", Float) = 0
        _Spacing ("Ring spacing (m)", Float) = 4
        _Spokes ("Spokes", Float) = 24
        _Hue ("Palette offset", Float) = 0.5
        _Intensity ("Intensity", Float) = 0.5
        _FadeK ("Distance fade", Float) = 0.014
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
            float _Radius, _Length, _Scroll, _Phase, _Wob, _BendAmp, _BendPhase, _Spacing, _Spokes, _Hue, _Intensity, _FadeK;
            CBUFFER_END

            struct A { float4 pos : POSITION; float2 uv : TEXCOORD0; };
            struct V { float4 pos : SV_POSITION; float2 uv : TEXCOORD0; float sp : TEXCOORD1; float3 wp : TEXCOORD2; };

            V vert(A i)
            {
                V o;
                float ang = i.uv.x * 6.2831853;
                float s = i.uv.y * _Length - _Length * 0.15;
                float sp = s + _Scroll;
                float r = _Radius * (1.0 + _Wob * sin(sp * 0.21 + _Phase) + 0.5 * _Wob * sin(sp * 0.53 - _Phase * 1.4));
                float2 bend = float2(sin(sp * 0.045 + _BendPhase), sin(sp * 0.037 + _BendPhase * 1.3 + 1.7)) * _BendAmp * saturate(s * 0.03);
                float3 p = float3(cos(ang) * r + bend.x, sin(ang) * r + bend.y, s);
                float3 wp = TransformObjectToWorld(p);
                o.pos = TransformWorldToHClip(wp);
                o.uv = i.uv;
                o.sp = sp;
                o.wp = wp;
                return o;
            }

            float Line(float x)
            {
                float d = abs(frac(x + 0.5) - 0.5);
                return 1.0 - smoothstep(0.0, max(fwidth(x), 1e-4) * 1.5, d);
            }

            float4 frag(V i) : SV_Target
            {
                float ring = Line(i.sp / _Spacing);
                float spoke = Line(i.uv.x * _Spokes);
                float lines = max(ring, spoke * 0.7);
                float dist = length(i.wp - _WorldSpaceCameraPos);
                float fade = exp(-dist * _FadeK) * smoothstep(0.0, 6.0, dist);
                float pulse = 0.55 + 0.45 * sin(i.sp * 0.35 - _Phase * 3.0);
                float3 c = TPalette(_Hue + i.uv.x * 0.25 + i.sp * 0.004);
                return float4(c * c * lines * (0.4 + pulse) * fade * _Intensity, 0);
            }
            ENDHLSL
        }
    }
}
