// unity_decks' DJ gear: one opaque lit shader for every part, picked by _Mode.
// Metal/rough GGX under four point lights (_GLPos/_GLCol, set by the show) and
// a procedural club environment for reflections (a warm softbox overhead, the
// palette glowing low behind). All patterns work in object space, so a part
// spinning by its transform carries its own grooves, dots and labels.
//  0 plain: _MainTex on upward faces (planar uv), _Side elsewhere; _Aniso > 0
//    brushes the metal round the object's y axis (spun aluminium).
//  1 vinyl: radial-streak groove highlight, track gaps, run-out, label (_MainTex).
//  2 strobe band: four rows of dots round the platter rim; crisp and red where
//    the illuminator lights them (_StrobeDir, world), smeared elsewhere.
//  3 knob: knurled skirt, white pointer line at object +z.
//  4 meter: a 15-segment LED ladder along uv.y lit to _Level.
//  5 screen: _MainTex as emission.
//  6 jog display: the round LCD in a media player's jog: a playhead marker
//    turning at _Smear revolutions, the track's progress (_Level) round the
//    rim, the cue point at the top.
//  7 player screen: _MainTex's text over a waveform the shader draws: the
//    zoomed waveform scrolling past the playhead at _Spin beats, beat grid,
//    a loop shaded while _Smear > 0.5, and the whole-track overview with
//    the progress (_Level).
//  8 jog ring: a dark glossy ring dimpled all round (30 dimples).
//  9 speaker cone: matte pressed paper, fine radial fibres and a few ribs.
// Any mode: _ExcR (inner, outer radius) > 0 moves vertices along object y by
// _Exc, fully inside the inner radius and fading to 0 at the outer: a
// loudspeaker cone and dust cap move rigidly while the surround stretches.
Shader "Trippin/HqGear"
{
    Properties
    {
        _MainTex ("Print", 2D) = "white" {}
        _UseTex ("Use print", Float) = 0
        _Tint ("Tint", Color) = (0.1, 0.1, 0.1, 1)
        _Side ("Side", Color) = (0.05, 0.05, 0.05, 1)
        _Metal ("Metal", Float) = 0
        _Rough ("Rough", Float) = 0.5
        _Aniso ("Aniso", Float) = 0
        _Mode ("Mode", Float) = 0
        _Emit ("Emit", Color) = (0, 0, 0, 0)
        _Level ("Level", Float) = 0
        _Spin ("Spin", Float) = 0
        _Smear ("Smear", Float) = 0
        _UVScale ("UV scale/offset", Vector) = (1, 1, 0, 0)
    }
    SubShader
    {
        Tags { "RenderType"="Opaque" "Queue"="Geometry" "RenderPipeline"="UniversalPipeline" }
        Pass
        {
            ZWrite On
            Cull Back
            HLSLPROGRAM
            #pragma vertex vert
            #pragma fragment frag
            #pragma target 4.5
            #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/Core.hlsl"
            #include "TrippinCommon.hlsl"

            TEXTURE2D(_MainTex); SAMPLER(sampler_MainTex);
            float4 _Tint, _Side, _Emit, _UVScale;
            float _UseTex, _Metal, _Rough, _Aniso, _Mode, _Level, _Spin, _Smear, _Exc;
            float4 _ExcR;
            // Shared by the show for every part.
            float4 _GLPos[4], _GLCol[4];
            float4 _GEnvA, _GEnvB, _GEnvC;      // softbox colour, low glow 1, low glow 2 (rgb, w = angle)
            float4 _StrobeDir;                  // xyz world direction from a platter's centre to its light
            float4 _StrobeRows;                 // per-row drift phase (dots stand still on the matching row)
            float4 _ArmA, _ArmB;                // the tonearm as a segment (world), for its shadow on the record

            struct A { float4 pos : POSITION; float3 n : NORMAL; float2 uv : TEXCOORD0; };
            struct V
            {
                float4 pos : SV_POSITION; float2 uv : TEXCOORD0; float3 wp : TEXCOORD1;
                float3 wn : TEXCOORD2; float3 op : TEXCOORD3; float3 on : TEXCOORD4;
                float3 wc : TEXCOORD5;   // the object's origin in world
                float3 wy : TEXCOORD6;   // the object's y axis in world
            };

            V vert(A i)
            {
                V o;
                if (_ExcR.y > 0.0)
                {
                    float rr0 = length(i.pos.xz);
                    i.pos.y += _Exc * saturate((_ExcR.y - rr0) / max(_ExcR.y - _ExcR.x, 1e-4));
                }
                o.wp = TransformObjectToWorld(i.pos.xyz);
                o.wn = TransformObjectToWorldNormal(i.n);
                o.op = i.pos.xyz; o.on = i.n;
                o.uv = i.uv * _UVScale.xy + _UVScale.zw;
                o.wc = TransformObjectToWorld(float3(0, 0, 0));
                o.wy = normalize(TransformObjectToWorldDir(float3(0, 1, 0)));
                o.pos = TransformWorldToHClip(o.wp);
                return o;
            }

            float GHash(float n) { return frac(sin(n * 12.9898) * 43758.5453); }

            // The club as seen in a reflection.
            float3 Env(float3 r)
            {
                float3 c = float3(0.006, 0.006, 0.008);
                // Softbox / booth lamp overhead.
                c += _GEnvA.rgb * pow(saturate(r.y), 6.0);
                // Club lights glowing low behind (two swinging hues), and the floor dark.
                float2 h = normalize(r.xz + 1e-4);
                float a1 = dot(h, float2(sin(_GEnvB.w), cos(_GEnvB.w)));
                float a2 = dot(h, float2(sin(_GEnvC.w), cos(_GEnvC.w)));
                float band = exp(-r.y * r.y * 18.0);
                c += _GEnvB.rgb * pow(saturate(a1), 8.0) * band;
                c += _GEnvC.rgb * pow(saturate(a2), 8.0) * band;
                c *= r.y < -0.1 ? 0.3 : 1.0;
                return c;
            }

            float3 Shade(float3 base, float metal, float rough, float3 n, float3 v, float3 wp, float3 t, float aniso)
            {
                float3 f0 = lerp(float3(0.04, 0.04, 0.04), base, metal);
                float3 diff = base * (1.0 - metal);
                float a = max(rough * rough, 0.002);
                float ax = a, ay = a;
                if (aniso > 0.0) { ax = max(a * (1.0 - aniso * 0.95), 0.001); ay = min(a * (1.0 + aniso * 4.0), 1.0); }
                float3 b = normalize(cross(n, t));
                float3 col = 0;
                float nv = saturate(dot(n, v)) + 1e-4;
                [unroll] for (int k = 0; k < 4; k++)
                {
                    float3 lv = _GLPos[k].xyz - wp;
                    float d2 = dot(lv, lv);
                    float3 l = lv * rsqrt(d2);
                    float att = _GLPos[k].w * _GLPos[k].w / (d2 + _GLPos[k].w * _GLPos[k].w * 0.05);
                    float nl = saturate(dot(n, l));
                    if (nl <= 0.0) continue;
                    float3 hh = normalize(l + v);
                    float th = dot(hh, t) / ax, bh = dot(hh, b) / ay, nh = dot(hh, n);
                    float dd = th * th + bh * bh + nh * nh;
                    float D = 1.0 / (3.14159 * ax * ay * dd * dd);
                    float3 F = f0 + (1.0 - f0) * pow(1.0 - saturate(dot(hh, v)), 5.0);
                    float Vis = 0.25 / max(max(nl, nv), 0.1);
                    col += _GLCol[k].rgb * att * nl * (diff / 3.14159 + D * F * Vis);
                }
                // Reflections of the club, blurred by roughness (fake: blend to the mean).
                float3 r = reflect(-v, n);
                float3 env = lerp(Env(r), Env(float3(0, 1, 0)) * 0.3 + 0.01, saturate(rough * 1.3));
                float3 Fv = f0 + (1.0 - f0) * pow(1.0 - nv, 5.0) * (1.0 - rough);
                col += env * Fv + diff * 0.02;
                return col;
            }

            float4 frag(V i) : SV_Target
            {
                float3 n = normalize(i.wn);
                float3 v = normalize(_WorldSpaceCameraPos - i.wp);
                float3 radial = i.wp - i.wc; radial -= i.wy * dot(radial, i.wy);
                float rr = length(i.op.xz);
                float3 tCirc = normalize(cross(i.wy, radial + 1e-5));
                int mode = (int)round(_Mode);
                float3 base = _Tint.rgb;
                float metal = _Metal, rough = _Rough, aniso = _Aniso;
                float3 t = tCirc;
                float3 emit = _Emit.rgb;

                if (mode == 0)
                {
                    if (_UseTex > 0.5 && i.on.y > 0.5) base = SAMPLE_TEXTURE2D(_MainTex, sampler_MainTex, i.uv).rgb;
                    else if (i.on.y <= 0.5) base = _Side.rgb;
                    if (_UseTex > 0.5 && i.on.y > 0.5) { metal = 0.0; rough = 0.55; aniso = 0.0; }
                }
                else if (mode == 1)
                {
                    // Vinyl: label inside r 0.5 (record units: 1 = 10 cm), grooves outside.
                    if (rr < 0.5)
                    {
                        float2 luv = i.op.xz / 1.0 + 0.5;
                        base = SAMPLE_TEXTURE2D(_MainTex, sampler_MainTex, luv).rgb;
                        metal = 0.0; rough = 0.6; aniso = 0.0;
                    }
                    else
                    {
                        base = float3(0.012, 0.012, 0.013);
                        metal = 0.0;
                        // Tracks: grooved bands with smooth gaps between them; a shiny run-out.
                        float trackR = (rr - 0.62) / (1.46 - 0.62);
                        float gap = 0.0;
                        [unroll] for (int k = 1; k < 5; k++) gap = max(gap, 1.0 - smoothstep(0.0, 0.006, abs(trackR - k * 0.2 + 0.03 * sin(k * 7.0))));
                        bool runout = rr < 0.62;
                        float micro = GHash(floor(rr * 900.0)) * 0.5 + 0.5;
                        rough = runout ? 0.18 : lerp(0.32, 0.12, gap) * micro;
                        aniso = runout ? 0.3 : lerp(0.92, 0.2, gap);
                        if (rr > 1.48) { rough = 0.3; aniso = 0.2; }   // the raised lip
                        t = tCirc;
                    }
                    // The tonearm's shadow from the key light (light 0).
                    float3 l0 = normalize(_GLPos[0].xyz - i.wp);
                    float tt = (_ArmA.y - i.wp.y) / max(l0.y, 1e-3);
                    float3 q = i.wp + l0 * tt;                       // where the ray meets the arm's height
                    float3 ab = _ArmB.xyz - _ArmA.xyz;
                    float h = saturate(dot(q - _ArmA.xyz, ab) / dot(ab, ab));
                    float dArm = length(q.xz - (_ArmA.xz + ab.xz * h));
                    float sh = 1.0 - 0.75 * (1.0 - smoothstep(0.02, 0.09, dArm)) * _ArmA.w;
                    float3 c = Shade(base, metal, rough, n, v, i.wp, t, aniso) * sh;
                    return float4(c, 1);
                }
                else if (mode == 2)
                {
                    // Strobe band: uv.y 0..1 across the band (4 rows), angle from object space.
                    float ang = atan2(i.op.z, i.op.x) / 6.2831853 + 0.5;
                    float row = floor(saturate(i.uv.y) * 3.999);
                    float rowY = frac(saturate(i.uv.y) * 3.999);
                    float counts[4] = { 180.0, 216.0, 133.0, 160.0 };
                    float cnt = row < 0.5 ? counts[0] : row < 1.5 ? counts[1] : row < 2.5 ? counts[2] : counts[3];
                    float drift = row < 0.5 ? _StrobeRows.x : row < 1.5 ? _StrobeRows.y : row < 2.5 ? _StrobeRows.z : _StrobeRows.w;
                    // Where the illuminator shines (world angle round the platter).
                    float3 rw = normalize(radial + 1e-5);
                    float lit = smoothstep(0.75, 0.97, dot(rw, normalize(_StrobeDir.xyz))) * _StrobeDir.w;
                    // Under the strobe the dots are frozen at the row's drift; elsewhere they smear.
                    float angW = atan2(rw.z, rw.x) / 6.2831853 + 0.5;   // frozen in the world, not on the platter
                    float phLit = frac(angW * cnt + drift);
                    float phMove = frac(ang * cnt);
                    float dLit = length(float2((phLit - 0.5) * 1.6, (rowY - 0.5) * 2.0));
                    float dMov = length(float2((phMove - 0.5) * 1.6, (rowY - 0.5) * 2.0));
                    float dotLit = 1.0 - smoothstep(0.55, 0.7, dLit);
                    float dotMov = lerp(1.0 - smoothstep(0.55, 0.7, dMov), 0.45 * (1.0 - smoothstep(0.45, 0.7, abs(rowY - 0.5) * 2.0)), saturate(_Smear));
                    float dt = lerp(dotMov, dotLit, lit);
                    base = lerp(float3(0.015, 0.015, 0.016), float3(0.55, 0.56, 0.58), dt);
                    metal = lerp(0.2, 0.9, dt); rough = 0.35;
                    aniso = 0.0;
                    emit += float3(1.0, 0.16, 0.04) * lit * (0.05 + 1.3 * dt);
                }
                else if (mode == 3)
                {
                    // Knob: knurled skirt, pointer line toward object +z.
                    float ang = atan2(i.op.x, i.op.z);
                    if (i.on.y < 0.5)
                    {
                        float k = sin(ang * 40.0);
                        n = normalize(n + tCirc * k * 0.35);
                    }
                    float ptr = (1.0 - smoothstep(0.035, 0.06, abs(ang))) * step(0.0, i.op.z);
                    ptr *= smoothstep(0.15, 0.4, rr / max(_Spin, 1e-3));   // keep the very centre clean
                    base = lerp(base, float3(0.85, 0.85, 0.85), ptr);
                    rough = lerp(rough, 0.4, ptr);
                }
                else if (mode == 4)
                {
                    float seg = floor(i.uv.y * 15.0);
                    float inSeg = frac(i.uv.y * 15.0);
                    float on = step(seg + 0.5, _Level * 15.0) * step(0.12, inSeg) * step(inSeg, 0.88);
                    float3 c = seg < 9.0 ? float3(0.15, 1.0, 0.25) : seg < 13.0 ? float3(1.0, 0.6, 0.05) : float3(1.0, 0.1, 0.06);
                    float3 off = c * 0.012;
                    return float4(lerp(off, c * 2.2, on), 1);
                }
                else if (mode == 6)
                {
                    float r = rr / 0.36;
                    float ang = atan2(i.op.x, i.op.z) / 6.2831853;            // 0 at the top, clockwise
                    float a01 = frac(ang + 1.0);
                    float3 c = float3(0.004, 0.01, 0.03);
                    // Progress round the rim: 60 segments.
                    float seg = frac(a01 * 60.0);
                    float rim = step(0.8, r) * step(r, 0.95) * step(0.15, seg) * step(seg, 0.85);
                    c += rim * lerp(float3(0.05, 0.08, 0.16), float3(0.75, 0.88, 1.2), step(a01, _Level));
                    // The playhead: a bar turning with the music.
                    float pa = frac(a01 - frac(_Smear) + 0.5) - 0.5;
                    float bar = (1.0 - smoothstep(0.006, 0.014, abs(pa))) * step(0.3, r) * step(r, 0.76);
                    c += bar * float3(1.6, 1.7, 1.9);
                    // The cue point at the top, and the inner ring.
                    c += (1.0 - smoothstep(0.01, 0.02, abs(frac(a01 + 0.5) - 0.5))) * step(0.96, r) * step(r, 1.0) * float3(2.2, 0.9, 0.1);
                    c += (1.0 - smoothstep(0.0, 0.015, abs(r - 0.28))) * float3(0.2, 0.35, 0.8);
                    return float4(c * _Emit.rgb + 0.002, 1);
                }
                else if (mode == 7)
                {
                    float2 uv = i.uv;
                    float3 c = 0;
                    // Zoomed waveform: 16 beats across, the playhead in the middle.
                    float b = _Spin + (uv.x - 0.5) * 16.0;
                    if (uv.y > 0.36 && uv.y < 0.88)
                    {
                        float sec = floor(b / 64.0);
                        float brk = step(GHash(sec * 3.7 + 1.0), 0.25) * step(1.0, sec);
                        float k = exp(-frac(b) * 7.0);
                        float hat = exp(-frac(b * 2.0 + 0.5) * 9.0) * 0.25;
                        float nz = GHash(floor(b * 48.0)) * 0.22;
                        float amp = lerp(saturate(0.22 + 0.55 * k + hat + nz), 0.12 + nz * 0.6, brk) * step(0.0, b);
                        float y = abs(uv.y - 0.62) / 0.26;
                        float body = step(y, amp);
                        float3 wc = lerp(float3(0.1, 0.32, 1.0), float3(0.85, 0.92, 1.0), saturate(k * 0.8 + (amp - y) * 0.5));
                        c += wc * body * (uv.x < 0.5 ? 0.75 : 1.0);
                        // Beat grid along the top and bottom, bars in red.
                        float bf = frac(b);
                        float grid = (1.0 - smoothstep(0.0, 0.02, min(bf, 1.0 - bf))) * step(0.82, abs(uv.y - 0.62) / 0.26 * 0.8 + 0.2);
                        float isBar = step(frac(b / 4.0), 0.25 / 4.0 + 0.001) + step(1.0 - 0.25 / 4.0, frac(b / 4.0));
                        c += grid * lerp(float3(0.5, 0.5, 0.55), float3(1.0, 0.15, 0.1), saturate(isBar));
                        // A loop: four beats shaded.
                        float l0 = floor(_Spin / 4.0) * 4.0;
                        c += step(0.5, _Smear) * step(l0, b) * step(b, l0 + 4.0) * float3(0.22, 0.12, 0.0);
                        // Playhead.
                        c += (1.0 - smoothstep(0.0, 0.0018, abs(uv.x - 0.5))) * float3(2.0, 2.0, 2.0);
                    }
                    // Overview: the whole track, played part greyed, a white needle.
                    if (uv.y > 0.21 && uv.y < 0.33)
                    {
                        float tb = uv.x * 1024.0;
                        float sec = floor(tb / 64.0);
                        float brk = step(GHash(sec * 3.7 + 1.0), 0.25) * step(1.0, sec);
                        float amp = lerp(0.45 + 0.4 * GHash(floor(tb / 2.0)), 0.18, brk);
                        float y = abs(uv.y - 0.27) / 0.06;
                        float3 oc = uv.x < _Level ? float3(0.25, 0.27, 0.32) : float3(0.15, 0.45, 1.0);
                        c += oc * step(y, amp);
                        c += (1.0 - smoothstep(0.0, 0.002, abs(uv.x - _Level))) * float3(2, 2, 2);
                    }
                    float3 t = SAMPLE_TEXTURE2D(_MainTex, sampler_MainTex, uv).rgb;
                    c = max(c, t);
                    return float4(c * _Emit.rgb + 0.003, 1);
                }
                else if (mode == 8)
                {
                    // Dimples round the ring: concave, so their normals lean in.
                    float ang = atan2(i.op.x, i.op.z);
                    float cell = frac(ang / 6.2831853 * 30.0) - 0.5;
                    float arc = cell * 6.2831853 / 30.0 * rr;                  // along the ring, units
                    float rad = (rr - 0.915) ;
                    float2 dd = float2(arc, rad) / 0.07;
                    float d = length(dd);
                    if (d < 1.0 && i.on.y > 0.3)
                    {
                        float3 radial3 = normalize(radial + 1e-5);
                        n = normalize(n - (tCirc * dd.x + radial3 * dd.y) * 0.55);
                        base = float3(0.05, 0.05, 0.055); rough = 0.25; metal = 0.6;
                    }
                }
                else if (mode == 9)
                {
                    float ang = atan2(i.op.x, i.op.z);
                    float fib = GHash(floor(ang * 900.0)) * 0.5 + GHash(floor(ang * 260.0 + rr * 30.0)) * 0.5;
                    base = _Tint.rgb * (0.8 + 0.35 * fib);
                    float rib = sin(rr * 95.0);
                    n = normalize(n + normalize(radial + 1e-5) * rib * 0.035);
                    metal = 0.0; rough = 0.75;
                }
                else if (mode == 5)
                {
                    float3 s = SAMPLE_TEXTURE2D(_MainTex, sampler_MainTex, i.uv).rgb;
                    return float4(s * _Emit.rgb + 0.004, 1);
                }

                float3 col = Shade(base, metal, rough, n, v, i.wp, t, aniso) + emit;
                return float4(col, 1);
            }
            ENDHLSL
        }
        Pass
        {
            Tags { "LightMode"="DepthOnly" }
            ZWrite On
            ColorMask R
            HLSLPROGRAM
            #pragma vertex vert
            #pragma fragment frag
            #pragma target 4.5
            #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/Core.hlsl"
            float _Exc; float4 _ExcR;
            float4 vert(float4 pos : POSITION) : SV_POSITION
            {
                if (_ExcR.y > 0.0) pos.y += _Exc * saturate((_ExcR.y - length(pos.xz)) / max(_ExcR.y - _ExcR.x, 1e-4));
                return TransformObjectToHClip(pos.xyz);
            }
            float4 frag() : SV_Target { return 0; }
            ENDHLSL
        }
    }
}
