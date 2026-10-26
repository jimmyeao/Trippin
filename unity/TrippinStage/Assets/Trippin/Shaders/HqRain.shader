// unity_rain's picture: a rainy night seen through a window. The street
// (_Street, an Alice-generated neon alley) is out of focus behind fogged
// glass; on the glass:
//  - small droplets at two scales, appearing and evaporating;
//  - running drops sliding down in fits and starts, wobbling sideways, each
//    wiping a clear trail through the condensation and leaving beads;
//  - every drop is a little lens: it shows the street sharp and upside
//    down, refracted through its curved surface, with a bright rim.
// _Run (drops running, from the music), _Fog (condensation), _Lights (the
// street's lights, from the energy) and _Pan (a slow drift of the view).
Shader "Trippin/HqRain"
{
    Properties
    {
        _Street ("Street", 2D) = "black" {}
        _Run ("Running drops", Float) = 1
        _Static ("Droplets", Float) = 1
        _Fog ("Condensation", Float) = 1
        _Blur ("Background blur", Float) = 4.5
        _Lights ("Lights", Float) = 1
        _Pan ("Pan", Vector) = (0, 0, 0, 0)
        _Gain ("Gain", Float) = 1
        _T ("Time", Float) = 0
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

            TEXTURE2D(_Street);
            SAMPLER(sampler_Street);
            CBUFFER_START(UnityPerMaterial)
            float _Run, _Static, _Fog, _Blur, _Lights, _Gain, _T;
            float4 _Pan;
            CBUFFER_END

            float H1(float2 p) { uint2 u = (uint2)(int2)p; uint x = u.x * 1664525u + u.y * 1013904223u + 374761393u; x ^= x >> 16; x *= 0x7feb352du; x ^= x >> 15; x *= 0x846ca68bu; x ^= x >> 16; return (x & 0xffffff) / 16777216.0; }
            float3 H3(float2 p) { return float3(H1(p), H1(p + 17.31), H1(p + 41.7)); }

            // A droplet's height: a smooth dome of radius r.
            float Dome(float2 d, float r) { float q = saturate(1.0 - dot(d, d) / (r * r)); return q * q * (3.0 - 2.0 * q); }

            // Small droplets that bead up and evaporate. p in screen units (y up, ~1 = screen height).
            float Droplets(float2 p, float scale, float t)
            {
                float2 g = p * scale;
                float2 id = floor(g), f = frac(g) - 0.5;
                float h = 0.0;
                // This cell and its neighbours (a droplet can cross the edge).
                for (int y = -1; y <= 1; y++)
                    for (int x = -1; x <= 1; x++)
                    {
                        float2 c = id + float2(x, y);
                        float3 n = H3(c);
                        float2 at = float2(x, y) + (n.xy - 0.5) * 0.7;
                        float life = frac(t * (0.04 + 0.06 * n.z) + n.x * 7.0);
                        float grow = saturate(life * 8.0) * saturate((1.0 - life) * 3.0);
                        float r = (0.12 + 0.22 * n.y) * grow;
                        h = max(h, Dome(f - at, r) * step(0.35, n.z));
                    }
                return h;
            }

            // Running drops: one per column, sliding down with a stop-start
            // motion, wobbling; returns the drop's height in .x and the
            // cleared trail (with its beads) in .y.
            float2 Runners(float2 p, float t)
            {
                const float cols = 9.0;
                float2 g = float2(p.x * cols, p.y);
                float col = floor(g.x);
                float3 n = H3(float2(col, 3.0));
                float lane = frac(g.x) - 0.5;
                // Each column's drop: falls from top to bottom over ~3-6 s,
                // pausing and lurching (a smoothstep staircase of its clock).
                float speed = 0.16 + 0.22 * n.y;
                float clk = t * speed * _Run + n.x * 9.0;
                float k = floor(clk), fr = frac(clk);
                float fall = k + smoothstep(0.15, 0.85, fr) * 0.9 + fr * 0.1;
                float y = 1.15 - frac(fall * 0.25 + n.z) * 1.5;           // its height on screen
                float wob = sin(p.y * 9.0 + n.x * 30.0) * 0.12 + sin(p.y * 23.0 + n.z * 11.0) * 0.05;
                float x = (n.z - 0.5) * 0.5 + wob;
                float2 d = float2((lane - x) / cols * 1.0, p.y - y);
                // The drop: a dome stretched down a little, its top flattened.
                // (taller than wide, flattened on top like a running bead)
                float drop = Dome(float2(d.x * 1.15, d.y * (d.y > 0.0 ? 1.1 : 0.75)), 0.03);
                // The trail above it: cleared glass, a few beads left behind.
                float above = saturate((p.y - y) * 8.0) * saturate(1.0 - (p.y - y) * 0.9);
                float trailW = 0.06 * (1.0 - saturate((p.y - y) * 0.6));
                float inLane = 1.0 - smoothstep(trailW * 0.6, trailW, abs(lane - x));
                float trail = inLane * above;
                float beadY = frac(p.y * 14.0 + n.y * 5.0);
                float bead = Dome(float2((lane - x) * 3.0, (beadY - 0.5) / 14.0 * 20.0), 0.18) * trail * step(0.5, H1(float2(col, floor(p.y * 14.0 + n.y * 5.0))));
                return float2(max(drop, bead * 0.6), trail);
            }

            float Height(float2 p, out float trail)
            {
                float2 r1 = Runners(p, _T);
                float2 r2 = Runners(p * float2(1.0, 1.0) + float2(0.37, 0.0), _T * 0.83 + 13.0);
                trail = max(r1.y, r2.y);
                float d = (Droplets(p, 18.0, _T) * 0.7 + Droplets(p + 3.1, 42.0, _T * 1.3) * 0.5) * _Static * (1.0 - trail);
                return max(max(r1.x, r2.x * 0.85), d);
            }

            float3 Street(float2 uv, float lod)
            {
                float3 c = SAMPLE_TEXTURE2D_LOD(_Street, sampler_Street, uv, lod).rgb;
                float l = dot(c, float3(0.299, 0.587, 0.114));
                // Neon and lamps glow (HDR) with the music's energy.
                return c * (0.6 + _Lights * 2.4 * smoothstep(0.5, 0.95, l) * l);
            }

            float4 frag(FsV i) : SV_Target
            {
                float aspect = _ScreenParams.x / _ScreenParams.y;
                float2 p = float2(i.ndc.x * aspect, i.ndc.y) * 0.5;          // screen, y up, height 1
                // The street behind: cover the screen, drifting slowly.
                float2 uv = i.ndc * 0.5 + 0.5;
                uv = (uv - 0.5) * 0.86 + 0.5 + _Pan.xy;

                float trail;
                float h = Height(p, trail);
                float e = 0.0015;
                float tx, ty;
                float hx = Height(p + float2(e, 0), tx), hy = Height(p + float2(0, e), ty);
                float2 grad = float2(hx - h, hy - h) / e;

                // Through a drop: a sharp, flipped view of the street (it's a
                // lens). The surface normal bends the ray: the offset grows
                // toward the rim, pointing back across the drop.
                float inDrop = smoothstep(0.02, 0.2, h);
                float3 nrm = normalize(float3(-grad * 0.02, 1.0));
                float2 duv = uv - nrm.xy * 0.07;
                // Condensation blurs the rest; trails and drops clear it.
                float fog = _Fog * (1.0 - trail) * (1.0 - inDrop);
                float lod = lerp(_Blur * 0.45, _Blur, fog);
                lod = lerp(lod, 0.8, inDrop);
                float3 c = Street(lerp(uv, duv, inDrop), lod);
                // Fogged glass scatters a little light: lift and soften.
                c = lerp(c, c * 0.8 + Street(uv, _Blur + 2.0) * 0.45, fog * 0.5);
                // A thin dark outline where the surface turns steeply away,
                // and a highlight on the top of each drop from the lights.
                float edge = saturate(1.0 - nrm.z * 1.08) * inDrop;
                c *= 1.0 - 0.55 * edge;
                float spec = pow(saturate(dot(nrm, normalize(float3(0.2, 0.5, 1.0)))), 60.0) * inDrop;
                c += spec * 0.6;
                return float4(c * _Gain, 1.0);
            }
            ENDHLSL
        }
    }
}
