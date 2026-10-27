// unity_stylus's picture: an extreme macro of a stylus riding a record groove,
// ray marched in one full-screen pass. World units are 100 µm: the record
// surface is the plane y = 0 with V grooves (90° walls) every _Pitch across x,
// running along z; each groove's walls swing sideways with the music
// (Wiggle: a sum of bands, their amplitudes in _Amp, scrolling with _Scroll
// as the record turns). The diamond tip sits in groove 0 at z = 0, following
// its wiggle, on an aluminium cantilever that rises out of frame. Vinyl is
// near-black with a rainbow sheen on the walls (the grooves diffract light),
// dust specks glint on the land. The camera is Unity's; the far surface fades
// into darkness (no depth of field on a full-screen pass).
Shader "Trippin/HqGroove"
{
    Properties
    {
        _Gain ("Gain", Float) = 1
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

            float _Gain, _Scroll, _Pitch, _Depth, _Dust, _Sheen, _Hue;
            float4 _Amp, _Amp2;      // band amplitudes (lateral, units)
            float4 _Key, _KeyCol, _Rim, _RimCol;

            float GH(float2 p) { return frac(sin(dot(p, float2(127.1, 311.7))) * 43758.5453); }

            // The groove's sideways swing at position s along it (s = z + scroll), groove g.
            float Wiggle(float s, float g)
            {
                s += g * 137.0;
                return _Amp.x * sin(s * 0.045 + 1.3) + _Amp.y * sin(s * 0.11 + 0.4) + _Amp.z * sin(s * 0.27 + 2.1)
                     + _Amp.w * sin(s * 0.63 + 0.7) + _Amp2.x * sin(s * 1.4 + 1.9) + _Amp2.y * sin(s * 3.1 + 0.2);
            }

            // Height of the vinyl: 0 on the land, down to -_Depth in a groove's V.
            float Height(float2 p, out float gx, out float g)
            {
                float s = p.y + _Scroll;
                float xr = p.x / _Pitch;
                g = floor(xr + 0.5);
                float w = Wiggle(s, g) / _Pitch;
                g = floor(xr - w + 0.5);
                w = Wiggle(s, g) / _Pitch;
                gx = (xr - w) - g;                       // -0.5..0.5 across the groove cell
                float hw = _Depth / _Pitch;              // 90° V: half width == depth
                float v = max(0.0, hw - abs(gx)) * _Pitch;
                // Dust: rare specks sitting on the land.
                float2 cell = floor(float2(p.x * 0.6, s * 0.6));
                float hd = GH(cell);
                float2 f = frac(float2(p.x * 0.6, s * 0.6)) - 0.5;
                float dust = hd > 1.0 - 0.012 * _Dust ? max(0.0, 0.12 - dot(f, f) * 1.6) * 3.0 : 0.0;
                return -v + dust;
            }

            float HeightOnly(float2 p) { float a, b; return Height(p, a, b); }

            // The stylus: a diamond cone in groove 0 at z 0, a cantilever rising back from it.
            float Stylus(float3 p, out int part)
            {
                float3 tip = float3(Wiggle(_Scroll, 0.0), -_Depth * 0.55, 0.0);   // riding groove 0
                float3 q = p - tip;
                // Diamond: a slim square shank (turned 45°) tapering to a rounded point,
                // set into the cantilever's end.
                float hgt = clamp(q.y, 0.0, 3.4);
                float r = min(hgt * 0.42 + 0.06, 0.62);
                float2 qr = float2(q.x + q.z, q.x - q.z) * 0.7071;
                float sq = max(abs(qr.x), abs(qr.y));
                float cone = max(sq - r, max(-q.y, q.y - 3.4)) * 0.8;
                // Cantilever: a thin tube from the shank's top, back and up out of frame.
                float3 a = tip + float3(0.0, 3.0, 0.0), b = tip + float3(0.0, 11.0, 26.0);
                float3 pa = p - a, ba = b - a;
                float h = saturate(dot(pa, ba) / dot(ba, ba));
                float tube = length(pa - ba * h) - lerp(0.42, 0.65, h);
                part = cone < tube ? 1 : 2;
                return min(cone, tube);
            }

            float3 Normal(float2 p)
            {
                float e = 0.02;
                float hx = HeightOnly(p + float2(e, 0)) - HeightOnly(p - float2(e, 0));
                float hz = HeightOnly(p + float2(0, e)) - HeightOnly(p - float2(0, e));
                return normalize(float3(-hx, 2.0 * e, -hz));
            }

            float3 Spec(float3 n, float3 v, float3 l, float rough)
            {
                float3 h = normalize(l + v);
                float a = rough * rough;
                float nh = saturate(dot(n, h));
                float d = a * a / (3.14159 * pow(nh * nh * (a * a - 1.0) + 1.0, 2.0) + 1e-5);
                return d * saturate(dot(n, l)) * 0.25;
            }

            float3 Rainbow(float t) { return 0.5 + 0.5 * cos(6.2831853 * (t + float3(0.0, 0.33, 0.67))); }

            float4 frag(FsV i) : SV_Target
            {
                float3 ro = _WorldSpaceCameraPos;
                float3 rd = normalize(i.rd);
                float3 col = float3(0.0, 0.0, 0.0);
                float t = 0.0;
                int hit = 0, part = 0;
                [loop] for (int k = 0; k < 64; k++)
                {
                    float3 p = ro + rd * t;
                    // Cheap bounds first: nothing on the record stands above 0.4 units, and
                    // the stylus fits in a 30-unit ball round the tip.
                    float hs = p.y > 0.9 ? p.y - 0.4 : p.y - HeightOnly(p.xz);
                    int pt = 0;
                    float dTip = length(p - float3(0.0, 4.0, 10.0)) - 24.0;
                    float ds = dTip > 0.5 ? dTip : Stylus(p, pt);
                    float d = min(hs * 0.45, ds);
                    if (ds < 0.01) { hit = 2; part = pt; break; }
                    if (hs < 0.004) { hit = 1; break; }
                    t += max(d, 0.004 + t * 0.006);
                    if (t > 100.0) break;
                }
                float3 p = ro + rd * t;
                float3 v = -rd;
                float3 lk = normalize(_Key.xyz - p), lr = normalize(_Rim.xyz - p);
                if (hit == 1)
                {
                    float gx, g;
                    float hgt = Height(p.xz, gx, g);
                    float3 n = Normal(p.xz);
                    bool wall = hgt < -0.002;
                    float3 base = float3(0.008, 0.008, 0.009);
                    float rough = wall ? 0.22 : 0.35;
                    col = base * (0.05 + saturate(dot(n, lk))) * _KeyCol.rgb;
                    col += Spec(n, v, lk, rough) * _KeyCol.rgb * 0.6 + Spec(n, v, lr, rough) * _RimCol.rgb * 0.5;
                    // The walls diffract the light into a rainbow that slides with the angle.
                    if (wall)
                    {
                        float a = dot(reflect(-lk, n), v);
                        col += Rainbow(a * 1.8 + gx * 0.6 + _Hue) * pow(saturate(a * 0.5 + 0.5), 6.0) * 0.12 * _Sheen * _KeyCol.rgb;
                    }
                    if (hgt > 0.002) col += float3(0.6, 0.58, 0.55) * saturate(dot(n, lk)) * 0.4;      // dust
                    // The stylus's shadow from the key light.
                    float sh = 1.0;
                    if (length(p.xz) < 14.0)
                    [loop] for (int s2 = 0; s2 < 14; s2++)
                    {
                        float3 q = p + lk * (0.05 + s2 * 0.8);
                        int pt2;
                        if (Stylus(q, pt2) < 0.02) { sh = 0.25; break; }
                    }
                    col *= sh;
                }
                else if (hit == 2)
                {
                    // Normal of the stylus SDF.
                    int dm;
                    float e = 0.01;
                    float3 n = normalize(float3(Stylus(p + float3(e, 0, 0), dm) - Stylus(p - float3(e, 0, 0), dm),
                                                Stylus(p + float3(0, e, 0), dm) - Stylus(p - float3(0, e, 0), dm),
                                                Stylus(p + float3(0, 0, e), dm) - Stylus(p - float3(0, 0, e), dm)));
                    if (part == 1)
                    {
                        // Diamond: sparkle and a little fire.
                        float3 r = reflect(-v, n);
                        float fres = pow(1.0 - saturate(dot(n, v)), 3.0);
                        col = Spec(n, v, lk, 0.08) * _KeyCol.rgb * 2.0 + Spec(n, v, lr, 0.08) * _RimCol.rgb * 1.5;
                        col += Rainbow(dot(r, lk) * 3.0 + _Hue) * 0.08 * (0.3 + fres);
                        col += float3(0.05, 0.05, 0.06) * fres;
                    }
                    else
                    {
                        // Brushed aluminium tube.
                        float3 base = float3(0.55, 0.56, 0.58);
                        col = base * saturate(dot(n, lk)) * _KeyCol.rgb * 0.25 + Spec(n, v, lk, 0.3) * _KeyCol.rgb * 0.9
                            + Spec(n, v, lr, 0.3) * _RimCol.rgb * 0.7;
                    }
                }
                // Far away the surface sinks into the dark (the macro's falloff).
                col *= exp(-t * 0.024);
                return float4(col * _Gain, 1.0);
            }
            ENDHLSL
        }
    }
}
