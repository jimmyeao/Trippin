// Lit armour for the procedural androids (RobotKit.cs): white ceramic, dark
// chrome and emissive parts, shaded as if in a studio rig with the stage's
// own colours: a white key softbox, a palette-coloured side light and back
// rim, an overhead strip, reflected in the polished surfaces, plus a coloured
// fresnel rim. Opaque. Parts set _Base / _Metal / _Emit / _EmitHue per
// material (or per part through a property block); the show sets _Gain from
// the eased loudness and _Dim (< 1) for the reflection under the floor.
Shader "Trippin/Robot"
{
    Properties
    {
        _Base ("Base colour", Color) = (0.85, 0.87, 0.9, 1)
        _Metal ("Metallic 0..1", Float) = 0.2
        _Rough ("Roughness (reflection blur) 0..1", Float) = 0.25
        _Emit ("Emission", Float) = 0
        _EmitHue ("Emission palette position", Float) = 0.5
        _Hue ("Rig palette position", Float) = 0.5
        _Gain ("Gain", Float) = 1
        _Dim ("Dim (reflection)", Float) = 1
    }
    SubShader
    {
        Tags { "RenderType"="Opaque" "Queue"="Geometry" "RenderPipeline"="UniversalPipeline" }
        ZWrite On
        Cull Back
        Pass
        {
            HLSLPROGRAM
            #pragma vertex vert
            #pragma fragment frag
            #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/Core.hlsl"
            #include "TrippinCommon.hlsl"

            CBUFFER_START(UnityPerMaterial)
            float4 _Base;
            float _Metal, _Rough, _Emit, _EmitHue, _Hue, _Gain, _Dim;
            CBUFFER_END

            struct A { float4 pos : POSITION; float3 nrm : NORMAL; };
            struct V { float4 pos : SV_POSITION; float3 n : TEXCOORD0; float3 wp : TEXCOORD1; };

            V vert(A i)
            {
                V o;
                float3 wp = TransformObjectToWorld(i.pos.xyz);
                o.pos = TransformWorldToHClip(wp);
                o.wp = wp;
                o.n = normalize(TransformObjectToWorldNormal(i.nrm));
                return o;
            }

            float3 Pal(float t) { float3 c = TPalette(t); return c * c; }

            float Box(float3 r, float3 dir, float lo, float hi) { return smoothstep(lo, hi, dot(r, normalize(dir))); }

            // The studio the metal reflects.
            float3 Studio(float3 r)
            {
                float3 e = lerp(float3(0.008, 0.01, 0.018), float3(0.05, 0.055, 0.07), saturate(r.y * 0.5 + 0.5));
                float soft = lerp(0.0, 0.07, _Rough * 4.0);
                e += float3(1.0, 1.0, 1.0) * 2.4 * Box(r, float3(-0.7, 0.5, -0.5), 0.86 - soft, 0.95);
                e += Pal(_Hue) * 2.0 * Box(r, float3(0.8, 0.2, 0.3), 0.8 - soft, 0.93);
                e += Pal(_Hue + 0.4) * 1.8 * Box(r, float3(-0.6, 0.3, 0.7), 0.8 - soft, 0.93);
                e += float3(0.9, 0.95, 1.0) * 1.2 * smoothstep(0.88 - soft, 1.0, r.y);
                e += Pal(_Hue + 0.2) * 0.35 * smoothstep(0.2, -0.6, r.y);      // the lit floor below
                return e;
            }

            float4 frag(V i) : SV_Target
            {
                float3 n = normalize(i.n);
                float3 v = normalize(_WorldSpaceCameraPos - i.wp);
                float ndv = saturate(dot(n, v));
                float3 r = reflect(-v, n);
                float fres = pow(1.0 - ndv, 4.0);
                float3 f0 = lerp(float3(0.04, 0.04, 0.04), _Base.rgb, _Metal);
                float3 F = f0 + (1.0 - f0) * fres;
                float3 spec = Studio(r) * F;
                float3 keyDir = normalize(float3(-0.7, 0.5, -0.5));
                float3 diff = _Base.rgb * (1.0 - _Metal) * (0.05 + 0.6 * saturate(dot(n, keyDir)) + 0.35 * saturate(dot(n, normalize(float3(0.8, 0.2, 0.3)))) * Pal(_Hue) * 2.0
                                                          + 0.3 * saturate(dot(n, normalize(float3(-0.6, 0.3, 0.7)))) * Pal(_Hue + 0.4) * 2.0);
                float3 rim = Pal(_Hue + 0.1) * pow(1.0 - ndv, 3.0) * 0.55;
                float3 col = (diff + spec + rim) * _Gain + Pal(_EmitHue) * _Emit;
                return float4(col * _Dim, 1.0);
            }
            ENDHLSL
        }
    }
}
