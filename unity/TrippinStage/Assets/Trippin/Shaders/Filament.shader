// Glowing polylines from a buffer of points (xyz + glow in w): lines of
// _LineLen points from _Base on, 6 vertices per segment, expanded into a
// ribbon that faces the camera. Soft across the width, additive, so
// overlapping light builds up. Colour from the palette along each line.
Shader "Trippin/Filament"
{
    Properties
    {
        _Width ("Width", Float) = 0.05
        _Gain ("Gain", Float) = 0.6
        _Hue ("Hue", Float) = 0.5
        _HueLine ("Hue per line", Float) = 0.01
        _HueAlong ("Hue along", Float) = 0.2
        _Taper ("Taper to the end", Float) = 0
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
            #pragma target 4.5
            #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/Core.hlsl"
            #include "TrippinCommon.hlsl"

            StructuredBuffer<float4> _P;
            uint _Base, _LineLen;
            float _Width, _Gain, _Hue, _HueLine, _HueAlong, _Taper;

            struct V { float4 pos : SV_POSITION; float side : TEXCOORD0; float3 col : TEXCOORD1; };

            V vert(uint vid : SV_VertexID)
            {
                V o;
                uint segs = _LineLen - 1;
                uint sg = vid / 6, c = vid % 6;
                uint ln = sg / segs, j = sg % segs;
                // corners: (end, side) per vertex of the two triangles
                uint endB = (c == 1 || c == 4 || c == 5) ? 1u : 0u;
                float side = (c == 2 || c == 3 || c == 5) ? 1.0 : -1.0;
                float4 a = _P[_Base + ln * _LineLen + j];
                float4 b = _P[_Base + ln * _LineLen + j + 1];
                float4 p = endB ? b : a;
                float t = (j + endB) / (float)segs;
                float3 dir = b.xyz - a.xyz;
                float3 view = GetCameraPositionWS() - p.xyz;
                float3 across = cross(dir, view);
                float al = length(across);
                across = al > 1e-6 ? across / al : float3(0, 1, 0);
                float w = _Width * (1.0 - _Taper * t);
                o.pos = TransformWorldToHClip(p.xyz + across * side * w);
                o.side = side;
                float3 hue = TPalette(_Hue + ln * _HueLine + t * _HueAlong);
                o.col = hue * hue * 1.6 * p.w * _Gain;
                return o;
            }

            float4 frag(V i) : SV_Target
            {
                float m = 1.0 - i.side * i.side;
                return float4(i.col * m * m, 0);
            }
            ENDHLSL
        }
    }
}
