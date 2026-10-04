// A deep-space nebula, drawn as a full-screen pass from the view direction (no
// geometry, so no edges): three domain-warped smoke layers at different
// parallax depths, each tied to its own slice of the spectrum, a bright core
// with a shockwave ring on drops, a faint galactic band and two layers of
// stars. The show eases and passes everything in; nothing reads raw audio.
//  _Gain  luminance (eased loudness, build dip, drop flare)
//  _Spin  yaw of the whole sky (phrase swing)      _Zoom  contracts on a build
//  _Drift smooth energy clock: the smoke slides through the warp field
Shader "Trippin/DeepSpace"
{
    Properties
    {
        _Hue ("Palette offset", Float) = 0.55
        _Gain ("Gain", Float) = 1
        _Zoom ("Zoom", Float) = 1
        _Spin ("Spin (rad)", Float) = 0
        _Drift ("Drift (clock)", Float) = 0
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

            CBUFFER_START(UnityPerMaterial)
            float _Hue, _Gain, _Zoom, _Spin, _Drift;
            CBUFFER_END

            float3 Pal(float t) { float3 c = TPalette(t); return c * c; }

            float3 RotY(float3 d, float a)
            {
                float c = cos(a), s = sin(a);
                return float3(c * d.x + s * d.z, d.y, -s * d.x + c * d.z);
            }

            // Sparse stars on a sphere: one candidate per 3D cell, at most one visible per pixel.
            float3 StarLayer(float3 d, float scale, float density, float twk)
            {
                float3 p = d * scale;
                float3 cell = floor(p);
                float3 f = frac(p);
                float2 key = cell.xy + cell.z * float2(37.7, 17.3);
                float h = THash(key);
                float h2 = THash(key * 1.7 + 5.0);
                float3 c = 0.25 + 0.5 * float3(THash(key + 9.1), THash(key + 3.3), THash(key + 7.7));
                float size = lerp(0.05, 0.2, h2 * h2);
                float s = smoothstep(size, 0.0, length(f - c)) * step(density, h);
                float tw = 1.0 + 0.35 * sin(twk * 0.6 + h * 60.0);
                return Pal(h2 * 0.6 + 0.5 + _Hue * 0.3) * s * tw * (0.5 + 1.8 * h2);
            }

            float4 frag(FsV i) : SV_Target
            {
                float3 d = normalize(i.rd);
                float3 dd = RotY(d, _Spin);
                float bass = _RxLvl.x, high = _RxLvl.w;
                float lum = _RxMisc.y, tension = _RxMisc.z, impact = _RxMisc.w;
                float clk = _Drift;
                float warp = 1.2 + 2.4 * bass + 1.2 * _RxLvl.y;   // shape: the smoke is stirred by the low end and mids

                float3 col = float3(0.004, 0.005, 0.012);
                [unroll]
                for (int L = 0; L < 3; L++)
                {
                    float s = _Zoom * (1.3 + 0.9 * L);
                    float3 p = dd * s * 2.2 + float3(L * 7.3, L * 3.1, L * 5.7)
                             + _WorldSpaceCameraPos * (0.004 * (L + 1))
                             + float3(clk * 0.0012 * (1.0 + 0.4 * L), clk * 0.0007, clk * 0.0009 * (1.0 - 0.3 * L));
                    float w = TFbm3(p * 0.7 + 3.1);
                    float n = TFbm3(p + w * warp);
                    float mask = smoothstep(0.40, 0.62, w);              // voids between the clouds
                    float body = smoothstep(0.52, 0.86, n) * mask;
                    float fil = pow(1.0 - abs(2.0 * n - 1.0), 6.0) * smoothstep(0.45, 0.7, n) * mask;
                    float spec = RxSpec(L * 0.3 + 0.08);
                    float a = 0.3 + 1.6 * spec;                  // each depth layer is a slice of the spectrum
                    col += Pal(_Hue + 0.14 * L + n * 0.3) * body * a * 0.28;
                    col += Pal(_Hue + 0.14 * L + 0.5) * fil * a * 0.8;
                }

                // Galactic band.
                float3 N = normalize(float3(0.3, 1.0, 0.2));
                float band = exp(-pow(dot(dd, N) * 4.0, 2.0));
                col += Pal(_Hue + 0.05) * band * (0.35 + TFbm3(dd * 6.0 + 2.0)) * 0.08;

                // Core: swells with the bass, with a ring that opens on a drop.
                float3 D0 = normalize(float3(0.15 * sin(_RxClk.z), 0.28, 1.0));
                float cd = dot(dd, D0);
                float ang = acos(clamp(cd, -1.0, 1.0));
                col += Pal(_Hue + 0.08) * (exp(-ang * 10.0) * 0.22 + exp(-ang * ang * 500.0) * 0.9) * (0.4 + 1.2 * bass);
                col += Pal(_Hue + 0.3) * exp(-pow((ang - (0.1 + 0.55 * impact)) * 30.0, 2.0)) * impact * 0.9;

                // Stars.
                float tw = _RxClk.y;
                col += (StarLayer(d, 58.0, 0.95, tw) + 0.7 * StarLayer(d, 130.0, 0.92, tw + 9.0)) * (0.5 + 1.0 * high + 0.4 * lum) * 2.5;

                col *= _Gain;
                col = col / (1.0 + 0.3 * col);
                return float4(col, 1.0);
            }
            ENDHLSL
        }
    }
}
