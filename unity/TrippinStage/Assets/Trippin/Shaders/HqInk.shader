// unity_ink's picture: drops of coloured ink falling into a tank of still
// water, ray marched as a volume. A drop (_Drop: x, z, age in seconds,
// size; the CPU packs the live ones first, _Count of them) forms a vortex
// ring as it lands: a torus that sinks, slows and spreads, trailing a thin
// stem back up to where it entered (_Shape: its head height, ring radius,
// tube thickness and lobe droop, worked out on the CPU). The ring is
// unstable: it breaks into lobes that sink ahead and trail veils (the
// jellyfish shape of real ink), billowing with turbulence that grows as it
// ages, its edges eroded into wisps; old ink loosens into clouds and fades.
// Each drop's colour (_DropCol) absorbs the light passing through it
// (Beer-Lambert) and forward-scatters a soft box above and behind the tank,
// self-shadowed by its own thickness, so thick ink is deep and saturated and its thin
// edges glow. Noise comes from the shared baked volume (HqNoise), and each
// ray marches only where it crosses a drop's bounds.
Shader "Trippin/HqInk"
{
    Properties
    {
        _T ("Time", Float) = 0
        _Density ("Density", Float) = 1
        _Billow ("Billow", Float) = 1
        _Glow ("Glow", Float) = 1
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

            CBUFFER_START(UnityPerMaterial)
            float _T, _Density, _Billow, _Glow, _Gain;
            CBUFFER_END
            float4 _Drop[8], _DropCol[8], _Shape[8];
            int _Count;
            float4 _Light;   // rgb soft box colour
            TEXTURE3D(_Vol); SAMPLER(sampler_Vol);

            static const float Top = 9.0;     // the water's surface
            static const float3 L = float3(-0.5, 0.65, 0.55) / 0.9644;   // toward the soft box

            float Fade(float age) { return saturate(1.0 - (age - 6.0) / 5.0) / (1.0 + age * 0.06); }

            float4 V(float3 p) { return SAMPLE_TEXTURE3D_LOD(_Vol, sampler_Vol, p * 0.0625, 0); }

            // One drop's density at p, given the shared turbulence w and fine
            // detail f (0..1). sh = the drop's shape (head height, ring radius,
            // tube thickness, the lobes' droop), worked out on the CPU.
            float Drop(float3 p, float4 d, float4 sh, float3 w, float f)
            {
                float age = d.z;
                float s = d.w;
                float3 head = float3(d.x, sh.x, d.y);
                float R = sh.y, r = sh.z;
                float3 q = p - head;
                float reach = (0.12 + 0.12 * age) * _Billow;
                // Outside the drop's reach: nothing (most taps end here).
                if (length(q.xz) > R + r + reach * 1.8 + 0.1 || q.y < -sh.w - r - reach * 1.8) return 0.0;
                q += w * reach;                                               // billows grow with age
                float seed = frac(d.x * 7.31 + d.y * 3.17) * 6.283;
                float ang = atan2(q.z, q.x);
                float lobe = 0.5 + 0.5 * cos(4.0 * ang + seed);
                lobe *= lobe;
                float droop = lobe * sh.w;
                float2 t = float2(length(q.xz) - R, q.y + droop);
                float ring = smoothstep(r, 0.0, length(t));
                // Veils hang from the ring down to each sinking lobe.
                float veil = saturate(1.0 - abs(t.x) / (r * 0.6)) * step(-droop, q.y) * step(q.y, 0.0) * lobe * 0.45;
                ring = saturate(max(ring, veil) * 1.3 - f * 0.45);     // eroded into wisps
                // The stem, curling more the deeper it reaches, thinning as it ages.
                float curl = saturate((Top - p.y) / (Top - head.y + 0.5));
                float2 sd = (p.xz + w.xz * (0.1 + 0.4 * curl) * _Billow) - head.xz;
                float stem = saturate(1.0 - length(sd) / (0.05 + 0.02 * age) / s) * step(head.y, p.y) * 0.45 * exp(-age * 0.18);
                return (ring + stem) * Fade(age);
            }

            // Ray vs box: entry and exit (exit < entry on a miss).
            float2 Box(float3 ro, float3 ird, float3 bmin, float3 bmax)
            {
                float3 t0 = (bmin - ro) * ird, t1 = (bmax - ro) * ird;
                float3 tn = min(t0, t1), tf = max(t0, t1);
                return float2(max(max(tn.x, tn.y), max(tn.z, 0.0)), min(min(tf.x, tf.y), tf.z));
            }

            float4 frag(FsV i) : SV_Target
            {
                float3 ro = _WorldSpaceCameraPos;
                float3 rd = normalize(i.rd);
                float3 ird = 1.0 / rd;
                // The water: dark, lighter toward the surface, the soft box
                // glowing through the back of the tank.
                float3 bg = lerp(float3(0.004, 0.006, 0.012), float3(0.03, 0.045, 0.07), saturate(rd.y * 1.5 + 0.4));
                float sb = saturate(dot(rd, L));
                bg += _Light.rgb * (pow(sb, 6.0) * 0.05 + pow(sb, 40.0) * 0.08);

                // The stretch of the ray that crosses any drop's bounds.
                float a = 1e5, b = 0.0;
                for (int j = 0; j < _Count; j++)
                {
                    float4 d = _Drop[j], sh = _Shape[j];
                    float e = sh.y + sh.z + (0.3 + 0.3 * d.z) * _Billow + 0.2;
                    // The ring and its lobes, then the thin column of the stem above.
                    float2 h = Box(ro, ird, float3(d.x - e, sh.x - sh.w - sh.z - e * 0.5, d.y - e), float3(d.x + e, sh.x + sh.z + e * 0.5, d.y + e));
                    if (h.y > h.x) { a = min(a, h.x); b = max(b, h.y); }
                    float es = 0.12 + 0.03 * d.z + 0.6 * _Billow;
                    h = Box(ro, ird, float3(d.x - es, sh.x, d.y - es), float3(d.x + es, Top, d.y + es));
                    if (h.y > h.x) { a = min(a, h.x); b = max(b, h.y); }
                }
                float3 col = 0;
                float3 trans = 1.0;
                if (b > a)
                {
                    // Steps of at least 0.14 m: a ray through one drop takes fewer.
                    float dt = max((b - a) / 44.0, 0.14);
                    int Steps = (int)ceil((b - a) / dt);
                    uint2 px = (uint2)i.pos.xy;
                    uint hsh = px.x * 1973u + px.y * 9277u;
                    hsh = (hsh ^ (hsh >> 15)) * 0x2c1b3c6du; hsh = (hsh ^ (hsh >> 12)) * 0x297a2d39u; hsh ^= hsh >> 15;
                    float t = a + dt * (hsh & 0xffffu) / 65536.0;
                    float cosv = dot(rd, L);
                    // Henyey-Greenstein forward scatter (g 0.55): ink between the eye
                    // and the soft box glows at its thin edges.
                    float phase = 0.35 + 0.325 * (1.0 - 0.3025) / pow(abs(1.3025 - 1.1 * cosv), 1.5);
                    for (int k = 0; k < Steps; k++)
                    {
                        float3 p = ro + rd * t;
                        float4 n0 = V(p * 0.9 + float3(0, _T * 0.15, 0));
                        float4 n1 = V(p * 2.3 + float3(5.1, 0, 9.7));
                        float3 w = (n0.rgb * 2.0 - 1.0) + (n1.rgb * 2.0 - 1.0) * 0.4;
                        float f = V(p * 3.2 + w * 0.7 + float3(0, _T * 0.3, 0)).a;
                        float3 colSum = 0;
                        float tot = 0.0;
                        for (int j = 0; j < _Count; j++)
                        {
                            float dj = Drop(p, _Drop[j], _Shape[j], w, f);
                            tot += dj;
                            colSum += _DropCol[j].rgb * dj;
                        }
                        if (tot > 0.001)
                        {
                            float3 c = colSum / tot;
                            float3 sigma = (1.0 - c) * 1.6 + 0.25;     // the ink takes out the light it isn't
                            // Self-shadow: the ink already crossed toward the eye stands in
                            // for the ink toward the light behind it (both lie back along
                            // the ray's side of the soft box), plus its own thickness.
                            float3 lit = exp(-sigma * tot * _Density * 0.9) * (0.35 + 0.65 * trans);
                            float3 stepT = exp(-sigma * tot * _Density * 3.0 * dt);
                            float amb = 0.12 + 0.18 * saturate(p.y / Top);
                            col += trans * (1.0 - stepT) * c * (_Light.rgb * lit * phase + amb) * _Glow;
                            trans *= stepT;
                            if (max(trans.r, max(trans.g, trans.b)) < 0.02) break;
                        }
                        t += dt;
                    }
                }
                col += trans * bg;
                return float4(col * _Gain, 1.0);
            }
            ENDHLSL
        }
    }
}
