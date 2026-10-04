// A flight down a snaking sci-fi corridor, drawn as a full-screen pass: each
// pixel's view ray is intersected with a bending, ribbed cylinder (a few
// refinement steps, no geometry), and the wall is shaded as a tiled panel
// wall. The camera sits on the axis at the origin looking down +z; _Scroll
// slides the corridor past it.
//  - Shape: the ribs (constrictions every 14 m) deepen with the eased bass, so
//    the corridor breathes; the wall panels are a live spectrum wrapped round
//    the circumference (each sector lit by its own band, mirrored top/bottom).
//  - Motion: _Scroll is the smooth energy clock (the flight surges on a drop),
//    the bends swing with the phrase, light packets run along the strips.
//  - Luminance: _Gain (eased loudness, build dip, drop flare). No beat-synced
//    flashes (a flight scene).
Shader "Trippin/Corridor"
{
    Properties
    {
        _Hue ("Palette offset", Float) = 0.5
        _Gain ("Gain", Float) = 1
        _Radius ("Radius (m)", Float) = 9
        _Scroll ("Scroll (m)", Float) = 0
        _Phase ("Packet phase", Float) = 0
        _BendAmp ("Bend (m)", Float) = 3
        _BendPhase ("Bend phase", Float) = 0
        _Rib ("Rib depth 0..1", Float) = 0.2
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
            float _Hue, _Gain, _Radius, _Scroll, _Phase, _BendAmp, _BendPhase, _Rib;
            CBUFFER_END

            static const float kTau = 6.2831853;
            static const float kNA = 32.0;       // panels round the circumference
            static const float kRingLen = 4.0;   // panel length (m)
            static const float kRibLen = 14.0;

            float3 Pal(float t) { float3 c = TPalette(t); return c * c; }

            float2 Bend(float z)
            {
                float sp = z + _Scroll;
                return float2(sin(sp * 0.045 + _BendPhase), sin(sp * 0.037 + _BendPhase * 1.3 + 1.7)) * _BendAmp * saturate(z * 0.03);
            }

            float Rib(float z)
            {
                float sp = z + _Scroll;
                return pow(0.5 + 0.5 * cos(sp * (kTau / kRibLen)), 6.0);
            }

            float RadiusAt(float z)
            {
                float sp = z + _Scroll;
                return _Radius * (1.0 - _Rib * Rib(z)) * (1.0 + 0.06 * sin(sp * 0.21 + _Phase));
            }

            float4 frag(FsV i) : SV_Target
            {
                float3 d = normalize(i.rd);
                float bass = _RxLvl.x, lum = _RxMisc.y, impact = _RxMisc.w;
                float2 dxy = d.xy;
                float a = max(dot(dxy, dxy), 1e-5);
                float t = _Radius / sqrt(a);
                [unroll]
                for (int k = 0; k < 4; k++)
                {
                    float z = max(d.z, 0.0) * t;
                    float2 c = Bend(z);
                    float R = RadiusAt(z);
                    float b = -2.0 * dot(dxy, c);
                    float cc = dot(c, c) - R * R;
                    float disc = max(b * b - 4.0 * a * cc, 0.0);
                    t = (-b + sqrt(disc)) / (2.0 * a);
                    t = min(t, 320.0);
                }
                float3 p = d * t;
                float2 c0 = Bend(p.z);
                float2 rel = p.xy - c0;
                float ang = atan2(rel.y, rel.x);
                float th = ang / kTau + 0.5;                 // 0..1 round the wall
                float sp = p.z + _Scroll;

                // Panel cell and its band (mirrored top / bottom so lows sit at the floor and ceiling).
                float u = th * kNA, v = sp / kRingLen;
                float ia = floor(u), ir = floor(v);
                float fa = frac(u), fr = frac(v);
                float bin = abs((ia + 0.5) / kNA * 2.0 - 1.0);
                float lvl = RxSpec(bin);
                float hh = THash(float2(ia, ir));

                // Anti-aliasing widths (the atan seam would break fwidth, so use the ray-hit derivatives).
                float2 rxd = ddx(rel), ryd = ddy(rel);
                float dth = (abs(rel.x * rxd.y - rel.y * rxd.x) + abs(rel.x * ryd.y - rel.y * ryd.x)) / max(dot(rel, rel), 1e-3);
                float wu = dth * kNA / kTau + 1e-3;
                float wv = fwidth(v) + 1e-3;
                float ea = min(fa, 1.0 - fa), er = min(fr, 1.0 - fr);
                float line_ = max(1.0 - smoothstep(0.0, 1.5 * wu + 0.02, ea), 1.0 - smoothstep(0.0, 1.5 * wv + 0.02, er));
                float inner = smoothstep(0.1, 0.2, ea) * smoothstep(0.1, 0.2, er);

                // Fade the fine pattern into its average as it shrinks below a pixel (far wall), instead of aliasing.
                float fpu = saturate(1.0 - wu * 2.0), fpv = saturate(1.0 - wv * 2.0);
                float fp = min(fpu, fpv);
                float pk = lerp(0.3, pow(0.5 + 0.5 * sin(sp * 0.35 - _Phase * 3.0), 3.0), fpv);
                float panel = lerp(0.5, inner * (0.25 + 0.9 * hh), fp);
                float3 wall = Pal(_Hue + th * 0.3 + sp * 0.002);

                // Panels: lit by their band, with a per-panel flicker of brightness (not of time).
                float3 col = wall * 0.012;
                col += wall * panel * lvl * (0.5 + 0.7 * pk) * 0.55;
                col += Pal(_Hue + 0.5) * line_ * (0.1 + 0.25 * lum + 0.3 * lvl);

                // Light strips along the corridor, packets sliding down them.
                float strip = lerp(0.2, exp(-pow((frac(th * 8.0 + 0.5) - 0.5) * 15.0, 2.0)), fpu);
                col += Pal(_Hue + 0.15) * strip * (0.15 + 1.0 * pk) * (0.4 + 1.0 * bass);

                // Rings at the ribs.
                float rib = Rib(p.z);
                col += Pal(_Hue + 0.3) * pow(rib, 12.0) * (0.15 + 0.7 * lum + 0.5 * impact) * exp(-t * 0.02);

                // Motes streaming past.
                float mote = step(0.985, THash(float2(floor(th * 64.0), floor(sp * 0.6)))) * smoothstep(0.35, 0.0, length(float2(frac(th * 64.0), frac(sp * 0.6)) - 0.5));
                col += mote * Pal(_Hue + 0.6) * 1.2 * (0.4 + _RxLvl.w);

                // Distance: fog into the palette, a bright flare where the corridor ends.
                float fog = exp(-t * 0.016);
                col = col * fog + Pal(_Hue + 0.1) * (1.0 - fog) * 0.05;
                float flare = exp((d.z - 1.0) * (80.0 / (1.0 + 2.0 * bass)));
                col += Pal(_Hue + 0.1) * flare * (0.5 + 1.8 * bass + impact);

                col *= _Gain;
                col = col / (1.0 + 0.3 * col);
                return float4(col, 1.0);
            }
            ENDHLSL
        }
    }
}
