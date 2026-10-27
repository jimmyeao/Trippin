// unity_gold's picture: molten gold pouring into a pool in a dark crucible,
// ray marched in one full-screen pass. The pool is a height field: rings
// travelling out from droplet impacts (_Ring[i]: x, z, start time, strength),
// a standing churn round where the stream lands, and a slow swell (_Swell).
// The stream is a wobbling column from above (_Stream: x, z, radius, wobble)
// with a splash crown where it meets the pool. Liquid gold is a near-perfect
// mirror of a warm studio (two softboxes overhead, a palette strip light low
// behind) and glows with its own heat, hottest where it's freshest: the
// stream and the churn. The crucible's rim is dark fired clay.
Shader "Trippin/HqGold"
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

            float _Gain, _T, _Swell, _Heat, _Hue;
            float4 _Ring[8];
            float4 _Stream;
            static const float PoolR = 9.0;

            float Pool(float2 p)
            {
                float h = 0.0;
                [unroll] for (int i = 0; i < 8; i++)
                {
                    float age = _T - _Ring[i].z;
                    if (age < 0.0 || age > 4.0) continue;
                    float r = length(p - _Ring[i].xy);
                    float front = age * 3.2;
                    float env = exp(-pow((r - front) / (0.5 + age * 0.35), 2.0)) * exp(-age * 1.1);
                    h += _Ring[i].w * 0.22 * env * sin((r - front) * 5.0);
                }
                // Churn round the stream's landing point, and a slow swell.
                float rs = length(p - _Stream.xy);
                h += 0.05 * exp(-rs * 0.45) * sin(rs * 6.0 - _T * 9.0) * (0.5 + _Stream.w);
                h += 0.04 * _Swell * sin(p.x * 0.5 + _T * 0.7) * sin(p.y * 0.43 - _T * 0.6);
                return h;
            }

            float StreamSdf(float3 p)
            {
                // A column falling from above, wobbling, narrowing as it falls, landing in a crown.
                float y = p.y;
                float wob = sin(y * 1.7 - _T * 6.0) * 0.06 * (1.0 + _Stream.w) + sin(y * 4.1 + _T * 9.0) * 0.025;
                float2 c = _Stream.xy + float2(wob, wob * 0.6) + float2(0.0, (y - 0.0) * 0.05);
                float r = _Stream.z * (0.75 + 0.25 * saturate(y / 8.0)) * (1.0 + 0.08 * sin(y * 3.0 - _T * 12.0));
                float col = max(length(p.xz - c) - r, -y - 0.3);
                // The crown: a ring of liquid thrown up round the landing point.
                float rr = length(p.xz - _Stream.xy);
                float crown = length(float2(rr - _Stream.z * 2.2, max(p.y - 0.05, 0.0) * 1.6)) - _Stream.z * 0.55 * (0.7 + 0.3 * sin(atan2(p.z - _Stream.y, p.x - _Stream.x) * 7.0 + _T * 5.0));
                return min(col, crown);
            }

            float Rim(float3 p)
            {
                float r = length(p.xz);
                return length(float2(r - PoolR - 0.6, p.y - 0.15)) - 0.8;
            }

            float3 Env(float3 r)
            {
                float3 c = float3(0.004, 0.003, 0.003);
                // Two softboxes overhead.
                float2 a = r.xz / max(r.y, 0.05);
                if (r.y > 0.1)
                {
                    // Soft-edged, like diffusion cloth.
                    float b1 = smoothstep(0.55, 0.35, abs(a.x + 0.9)) * smoothstep(0.8, 0.55, abs(a.y - 0.3));
                    float b2 = smoothstep(0.4, 0.22, abs(a.x - 1.1)) * smoothstep(0.5, 0.3, abs(a.y + 0.6));
                    c += (float3(1.0, 0.86, 0.68) * 1.1 * b1 + float3(1.0, 0.9, 0.78) * 0.7 * b2) * smoothstep(0.1, 0.4, r.y);
                }
                // A strip light low behind, in the palette.
                float3 pal = TPalette(_Hue); pal *= pal;
                c += pal * 1.6 * exp(-pow(r.y - 0.08, 2.0) * 400.0) * smoothstep(0.2, 0.6, r.z);
                return c;
            }

            float3 Hot(float t) { return float3(1.0, 0.42, 0.08) * t + float3(0.6, 0.35, 0.05) * t * t * t; }

            float4 frag(FsV i) : SV_Target
            {
                float3 ro = _WorldSpaceCameraPos;
                float3 rd = normalize(i.rd);
                float t = 0.0;
                int hit = 0;
                [loop] for (int k = 0; k < 72; k++)
                {
                    float3 p = ro + rd * t;
                    float hp = p.y > 0.6 ? p.y - 0.4 : p.y - Pool(p.xz);
                    float ds = StreamSdf(p), dr = Rim(p);
                    float d = min(hp * 0.6, min(ds * 0.8, dr));
                    if (ds < 0.004) { hit = 2; break; }
                    if (dr < 0.004) { hit = 3; break; }
                    if (hp < 0.004) { hit = length(p.xz) < PoolR + 0.3 ? 1 : 3; break; }
                    t += max(d, 0.003 + t * 0.004);
                    if (t > 55.0) break;
                }
                float3 p = ro + rd * t;
                float3 v = -rd;
                float3 col = 0;
                if (hit == 1 || hit == 2)
                {
                    float3 n;
                    if (hit == 1)
                    {
                        float e = 0.02;
                        n = normalize(float3(Pool(p.xz - float2(e, 0)) - Pool(p.xz + float2(e, 0)), 2.0 * e,
                                             Pool(p.xz - float2(0, e)) - Pool(p.xz + float2(0, e))));
                    }
                    else
                    {
                        float e = 0.01;
                        n = normalize(float3(StreamSdf(p + float3(e, 0, 0)) - StreamSdf(p - float3(e, 0, 0)),
                                             StreamSdf(p + float3(0, e, 0)) - StreamSdf(p - float3(0, e, 0)),
                                             StreamSdf(p + float3(0, 0, e)) - StreamSdf(p - float3(0, 0, e))));
                    }
                    float3 gold = float3(1.0, 0.76, 0.33);
                    float nv = saturate(dot(n, v));
                    float3 F = gold + (1.0 - gold) * pow(1.0 - nv, 5.0);
                    col = Env(reflect(-v, n)) * F;
                    // Heat: the stream and the churn glow; the pool cools away from them, a faint skin.
                    float rs = length(p.xz - _Stream.xy);
                    float heat = hit == 2 ? 0.85 : 0.05 + 0.7 * exp(-rs * 0.55);
                    float skin = 0.5 + 0.5 * sin(p.x * 3.1 + sin(p.z * 2.3 + _T * 0.4) * 2.0) * sin(p.z * 2.7 - _T * 0.3);
                    heat *= lerp(1.0, 0.75 + 0.25 * skin, saturate(rs / 6.0));
                    col += Hot(heat * _Heat) * (0.35 + 0.65 * (1.0 - nv)) * 0.9;
                }
                else if (hit == 3)
                {
                    // Fired clay, lit by the gold's glow.
                    float e = 0.01;
                    float3 n = normalize(float3(Rim(p + float3(e, 0, 0)) - Rim(p - float3(e, 0, 0)), Rim(p + float3(0, e, 0)) - Rim(p - float3(0, e, 0)), Rim(p + float3(0, 0, e)) - Rim(p - float3(0, 0, e))));
                    float clay = 0.03 + 0.02 * frac(sin(dot(floor(p.xz * 6.0), float2(12.9, 78.2))) * 43758.5);
                    col = clay * (Env(n) * 0.3 + Hot(_Heat * 0.5) * saturate(-dot(n, normalize(float3(p.x, 0, p.z)))) * 0.8);
                }
                else col = Env(rd) * 0.05;
                col *= exp(-t * 0.01);
                return float4(col * _Gain, 1.0);
            }
            ENDHLSL
        }
    }
}
