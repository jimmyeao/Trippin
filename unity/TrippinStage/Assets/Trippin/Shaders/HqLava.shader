// unity_lava's picture: three lava lamps on a glossy dark table, ray marched
// in one full-screen pass. Each lamp is a revolved glass body on a brushed
// metal base under a cap; inside it, blobs of wax (_Blob, lamp-local: xyz,
// radius) melt into one another and into the molten pool at the bottom
// (a smooth union), glowing from within as if lit through their own
// thickness, in a liquid that glows with its own colour. The glass adds
// Fresnel reflections and a tall highlight; the table reflects the lamps
// and pools their light; the wall behind catches a soft glow.
//  _Lamp[i]   xyz base position, w glass height
//  _Wax[i]    rgb wax colour, w liquid hue shift
//  _Liquid[i] rgb liquid colour
Shader "Trippin/HqLava"
{
    Properties
    {
        _Gain ("Gain", Float) = 1
        _Glow ("Glow", Float) = 1
        _Melt ("Melt", Float) = 0.25
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
            #pragma target 4.5
            #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/Core.hlsl"
            #include "TrippinCommon.hlsl"
            #include "TrippinFull.hlsl"

            float _Gain, _Glow, _Melt;
            float4 _Lamp[3], _Wax[3], _Liquid[3];
            float4 _Blob[24];      // 8 per lamp

            static const float BaseH = 0.38, CapH = 0.42;

            // The glass radius at height u (0 bottom .. 1 top of the glass).
            float GlassR(float u) { return 0.2 + 0.3 * sin(3.14159 * (saturate(u) * 0.8 + 0.1)); }

            float SMin(float a, float b, float k)
            {
                float h = saturate(0.5 + 0.5 * (b - a) / k);
                return lerp(b, a, h) - k * h * (1.0 - h);
            }

            // The lamp's outside (glass, base, cap) as a distance (lamp-local p).
            float Outer(float3 p, float H, out int part)
            {
                float rr = length(p.xz);
                // Base: a cone from the table up to the glass.
                float ub = saturate(p.y / BaseH);
                float base = max(rr - lerp(0.46, 0.3, ub), max(-p.y, p.y - BaseH));
                // Glass.
                float u = (p.y - BaseH) / H;
                float glass = max((rr - GlassR(u)) * 0.8, max(BaseH - p.y, p.y - BaseH - H));
                // Cap.
                float uc = saturate((p.y - BaseH - H) / CapH);
                float cap = max(rr - lerp(0.29, 0.12, uc), max(BaseH + H - p.y, p.y - BaseH - H - CapH));
                float d = min(base, min(glass, cap));
                part = d == glass ? 1 : 0;
                return d;
            }

            // The wax inside lamp i (lamp-local p).
            float Wax(float3 p, int i, float H)
            {
                float d = p.y - (BaseH + 0.08 + _Melt * 0.12);      // the molten pool at the bottom
                d = max(d, length(p.xz) - 0.45);
                [unroll] for (int k = 0; k < 8; k++)
                {
                    float4 b = _Blob[i * 8 + k];
                    float3 q = p - b.xyz;
                    // Blobs stretch a little along their travel.
                    q.y *= 0.85;
                    d = SMin(d, length(q) - b.w, 0.12 + _Melt * 0.1);
                }
                // Stay inside the glass.
                float u = (p.y - BaseH) / H;
                return max(d, length(p.xz) - GlassR(u) + 0.03);
            }

            float3 Room(float3 rd)
            {
                // A dark room: a warm glow on the wall behind the lamps.
                float3 c = lerp(float3(0.004, 0.003, 0.006), float3(0.03, 0.018, 0.02), saturate(rd.y * 0.5 + 0.6));
                return c;
            }

            // March the lamps along a ray; returns colour, and whether it hit (a).
            float4 Lamps(float3 ro, float3 rd)
            {
                float t = 0.0;
                int hitLamp = -1, part = 0;
                float3 p = ro;
                for (int s = 0; s < 72; s++)
                {
                    p = ro + rd * t;
                    float d = 1e5;
                    [unroll] for (int i = 0; i < 3; i++)
                    {
                        int pt;
                        float di = Outer(p - _Lamp[i].xyz, _Lamp[i].w, pt);
                        if (di < d) { d = di; hitLamp = i; part = pt; }
                    }
                    if (d < 0.002) break;
                    t += d;
                    if (t > 20.0) return 0;
                }
                if (t > 20.0) return 0;
                int i = hitLamp;
                float3 lp = p - _Lamp[i].xyz;
                float H = _Lamp[i].w;
                // The surface normal (central differences on Outer).
                float2 e = float2(0.002, 0);
                int dummy;
                float3 n = normalize(float3(Outer(lp + e.xyy, H, dummy) - Outer(lp - e.xyy, H, dummy),
                                            Outer(lp + e.yxy, H, dummy) - Outer(lp - e.yxy, H, dummy),
                                            Outer(lp + e.yyx, H, dummy) - Outer(lp - e.yyx, H, dummy)));
                float cosT = saturate(dot(-rd, n));
                float F = 0.04 + 0.96 * pow(1.0 - cosT, 5.0);
                float3 wax = _Wax[i].rgb, liq = _Liquid[i].rgb;
                if (part == 0)
                {
                    // Brushed metal, catching the lamp's own glow and the room.
                    float3 r = reflect(rd, n);
                    float3 c = Room(r) * 2.0 + wax * 0.12 * saturate(r.y * 2.0 + 0.5) * _Glow;
                    c += pow(saturate(dot(r, normalize(float3(-0.4, 0.7, -0.6)))), 40.0) * 0.6;
                    return float4(c * (0.55 + 0.45 * F), 1);
                }
                // Glass: march inside along a slightly bent ray.
                float3 rdi = normalize(rd - n * 0.15);
                float3 q = lp + rdi * 0.004;
                float3 col = 0;
                float travel = 0.0;
                bool hitWax = false;
                for (int k = 0; k < 40; k++)
                {
                    float u = (q.y - BaseH) / H;
                    if (length(q.xz) > GlassR(u) + 0.01 || u < 0.0 || u > 1.0) break;
                    float d = Wax(q, i, H);
                    if (d < 0.003) { hitWax = true; break; }
                    float st = max(d * 0.9, 0.012);
                    q += rdi * st;
                    travel += st;
                }
                // The liquid glows with the light passing through it: the chord
                // through the glass (analytic, so it can't band), half of it in
                // front of wax.
                float2 o2 = lp.xz, d2 = normalize(rdi.xz + 1e-5);
                float bb = dot(o2, d2), cc = dot(o2, o2) - pow(GlassR((lp.y - BaseH) / H), 2.0);
                float chord = 2.0 * sqrt(max(bb * bb - cc, 0.0)) * length(rdi.xz);
                col += liq * chord * (hitWax ? 0.4 : 0.9) * _Glow;
                if (hitWax)
                {
                    // Wax lit from within: hotter low down (the lamp's bulb is in the base),
                    // brighter where it's thin at the edges.
                    float2 g = float2(0.004, 0);
                    float3 wn = normalize(float3(Wax(q + g.xyy, i, H) - Wax(q - g.xyy, i, H),
                                                 Wax(q + g.yxy, i, H) - Wax(q - g.yxy, i, H),
                                                 Wax(q + g.yyx, i, H) - Wax(q - g.yyx, i, H)));
                    float edge = 1.0 - saturate(dot(-rdi, wn));
                    float heat = 1.0 - saturate((q.y - BaseH) / H) * 0.55;
                    col += wax * (0.55 + 0.9 * edge * edge) * heat * 1.6 * _Glow;
                }
                // The glass over it: reflections and a tall highlight.
                float3 r = reflect(rd, n);
                col = col * (1.0 - F) + Room(r) * F * 3.0;
                float hl = pow(saturate(dot(r, normalize(float3(-0.55, 0.15, -0.8)))), 60.0);
                col += hl * 1.4;
                return float4(col, 1);
            }

            float4 frag(FsV i) : SV_Target
            {
                float3 ro = _WorldSpaceCameraPos;
                float3 rd = normalize(i.rd);
                float3 col = Room(rd);
                float4 l = Lamps(ro, rd);
                if (l.a > 0.0) col = l.rgb;
                else if (rd.y < 0.0)
                {
                    // The table: glossy and dark, mirroring the lamps a little rippled,
                    // their light pooling round each base.
                    float tf = -ro.y / rd.y;
                    float3 fp = ro + rd * tf;
                    float3 rr = reflect(rd, float3(0, 1, 0));
                    rr.x += sin(fp.x * 9.0) * 0.004;
                    float4 m = Lamps(fp + rr * 0.01, rr);
                    col = float3(0.006, 0.005, 0.006) + m.rgb * 0.35 * m.a;
                    [unroll] for (int k = 0; k < 3; k++)
                    {
                        float d = length(fp.xz - _Lamp[k].xz);
                        col += _Wax[k].rgb * exp(-d * d * 1.6) * 0.18 * _Glow;
                    }
                    col *= exp(-tf * 0.03);
                }
                else
                {
                    // The wall catching the lamps' glow.
                    [unroll] for (int k = 0; k < 3; k++)
                    {
                        float3 toL = normalize(_Lamp[k].xyz + float3(0, 1.2, 0) - ro);
                        float c = saturate(dot(rd, toL));
                        col += _Wax[k].rgb * pow(c, 30.0) * 0.08 * _Glow + _Liquid[k].rgb * pow(c, 6.0) * 0.03 * _Glow;
                    }
                }
                return float4(col * _Gain, 1.0);
            }
            ENDHLSL
        }
    }
}
