// unity_kaleido's picture: a real kaleidoscope's mirrors over a live 3D
// image. _Src is a hidden camera's view into a chamber of tumbling glass
// shards (KaleidoShow.cs). Two mirror systems, crossfaded by _Mode:
//  0  N mirrors round the axis (_Seg segments, each mirrored): a mandala
//  1  three mirrors in a triangle: the image repeats over the whole field
// The tube's edge darkens and catches a rim of light; colours split a
// little toward the edge, like light through a glass rod.
Shader "Trippin/HqKaleido"
{
    Properties
    {
        _Src ("Source", 2D) = "black" {}
        _Seg ("Segments", Float) = 8
        _Mode ("Mirror mode", Float) = 0
        _Rot ("Rotation", Float) = 0
        _Zoom ("Zoom", Float) = 1
        _Tri ("Triangle scale", Float) = 2.2
        _Split ("Colour split", Float) = 1
        _Gain ("Gain", Float) = 1
        _Raw ("Raw (debug)", Float) = 0
    }
    SubShader
    {
        Tags { "RenderType"="Opaque" "Queue"="Background" "RenderPipeline"="UniversalPipeline" }
        ZTest Always
        ZWrite Off
        Cull Off
        Pass
        {
            HLSLPROGRAM
            #pragma vertex FsVert
            #pragma fragment frag
            #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/Core.hlsl"
            #include "TrippinCommon.hlsl"
            #include "TrippinFull.hlsl"

            TEXTURE2D(_Src);
            SAMPLER(sampler_Src);
            CBUFFER_START(UnityPerMaterial)
            float _Seg, _Mode, _Rot, _Zoom, _Tri, _Split, _Gain, _Raw;
            CBUFFER_END

            float2 Rot(float2 p, float a) { float c = cos(a), s = sin(a); return float2(c * p.x - s * p.y, s * p.x + c * p.y); }

            // N mirrors round the centre: fold the angle into one mirrored wedge.
            float2 Radial(float2 p)
            {
                float seg = 6.2831853 / max(_Seg, 2.0);
                float a = atan2(p.y, p.x);
                a = abs(fmod(a + 100.0 * seg, seg) - seg * 0.5);
                return float2(cos(a), sin(a)) * length(p);
            }

            // Three mirrors in an equilateral triangle: a triangle-wave fold
            // in the skewed lattice keeps every edge a mirror.
            float2 Triangle(float2 p)
            {
                const float s3 = 1.7320508;
                p *= _Tri;
                float2 q = float2(p.x - p.y / s3, 2.0 * p.y / s3);
                q = 1.0 - abs(frac(q * 0.5) * 2.0 - 1.0);
                if (q.x + q.y > 1.0) q = 1.0 - q.yx;
                float2 c = float2(q.x + q.y * 0.5, q.y * s3 * 0.5);
                return (c - float2(0.5, 0.2887)) / _Tri * 1.4;
            }

            float3 Look(float2 p, float split)
            {
                float2 f = lerp(Radial(p), Triangle(p), _Mode);
                f = Rot(f, _Rot * 0.37) / _Zoom;
                float2 uv = f * 0.5 + 0.5;
                // A little dispersion: red and blue fold from slightly different radii.
                float3 c;
                c.r = SAMPLE_TEXTURE2D_LOD(_Src, sampler_Src, 0.5 + (uv - 0.5) * (1.0 + split), 0).r;
                c.g = SAMPLE_TEXTURE2D_LOD(_Src, sampler_Src, uv, 0).g;
                c.b = SAMPLE_TEXTURE2D_LOD(_Src, sampler_Src, 0.5 + (uv - 0.5) * (1.0 - split), 0).b;
                return c;
            }

            float4 frag(FsV i) : SV_Target
            {
                if (_Raw > 0.5) return float4(SAMPLE_TEXTURE2D_LOD(_Src, sampler_Src, i.ndc * 0.5 + 0.5, 0).rgb, 1);
                float2 p = i.ndc;
                p.x *= _ScreenParams.x / _ScreenParams.y;
                p = Rot(p, _Rot);
                float r = length(p);
                float split = 0.012 * _Split * r;
                float3 c = Look(p, split);
                // The tube: dark beyond its rim, a thin bright ring at the edge.
                float tube = 1.32;
                float inside = 1.0 - smoothstep(tube - 0.06, tube + 0.02, r);
                float ring = exp(-pow((r - tube) / 0.012, 2.0)) * 0.35;
                c = c * inside + ring * float3(1.0, 0.95, 0.9);
                // Corners outside the tube keep a dim ghost of the image.
                c += Look(p * 0.6, split) * (1.0 - inside) * 0.08;
                return float4(c * _Gain, 1.0);
            }
            ENDHLSL
        }
    }
}
