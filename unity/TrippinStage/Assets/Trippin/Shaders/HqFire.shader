// unity_fire's flames: stage flame cannons, ray marched as volumes. Up to
// eight jets stand along the stage (_Jet: x, z, height, width); each is a
// column of turbulent fire (3D noise streaming upward) whose temperature
// falls with height and toward its edges, coloured on a blackbody ramp:
// white-yellow at the root, orange, deep red, then smoky dark at the tips.
// Drawn additively over the stage after everything else (the floor stops
// the rays). _T runs the turbulence; _Heat scales the brightness.
Shader "Trippin/HqFire"
{
    Properties
    {
        _T ("Time", Float) = 0
        _Heat ("Heat", Float) = 1
        _Turb ("Turbulence", Float) = 1
    }
    SubShader
    {
        Tags { "RenderType"="Transparent" "Queue"="Transparent+10" "RenderPipeline"="UniversalPipeline" }
        Blend One One
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
            float _T, _Heat, _Turb;
            CBUFFER_END
            float4 _Jet[8];
            int _JetCount;

            float Hash3(float3 p) { p = frac(p * 0.3183099 + 0.1); p *= 17.0; return frac(p.x * p.y * p.z * (p.x + p.y + p.z)); }
            float Noise(float3 x)
            {
                float3 i = floor(x), f = frac(x);
                f = f * f * (3.0 - 2.0 * f);
                return lerp(lerp(lerp(Hash3(i), Hash3(i + float3(1, 0, 0)), f.x), lerp(Hash3(i + float3(0, 1, 0)), Hash3(i + float3(1, 1, 0)), f.x), f.y),
                            lerp(lerp(Hash3(i + float3(0, 0, 1)), Hash3(i + float3(1, 0, 1)), f.x), lerp(Hash3(i + float3(0, 1, 1)), Hash3(i + float3(1, 1, 1)), f.x), f.y), f.z);
            }
            float Fbm(float3 p)
            {
                float v = 0.0, a = 0.5;
                for (int i = 0; i < 4; i++) { v += Noise(p) * a; p = p * 2.07 + 13.1; a *= 0.5; }
                return v;
            }

            // Blackbody-ish ramp: 0 cold (dark red) .. 1 hot (white-yellow).
            float3 Ramp(float t)
            {
                t = saturate(t);
                float3 c = float3(1.0, 0.18, 0.02) * smoothstep(0.0, 0.35, t);
                c = lerp(c, float3(1.0, 0.55, 0.1), smoothstep(0.3, 0.65, t));
                c = lerp(c, float3(1.0, 0.92, 0.6), smoothstep(0.6, 1.0, t));
                return c * (0.3 + 2.8 * t * t);
            }

            // Temperature at p of jet j (0 outside it).
            float Temp(float3 p, float4 jet)
            {
                float h = jet.z;
                if (h < 0.05) return 0.0;
                float y = p.y / h;                     // 0 at the nozzle, 1 at the tip
                if (y < 0.0 || y > 1.35) return 0.0;
                float2 d = p.xz - jet.xy;
                // The plume widens as it rises, then frays out at the top.
                float r = jet.w * (0.45 + 0.8 * y);
                float3 q = float3(d.x, p.y - _T * 7.0, d.y) * 0.55 * _Turb;
                float n = Fbm(q + float3(jet.x * 3.1, 0.0, 0.0));
                float lean = (n - 0.5) * 1.8 * y;
                float core = 1.0 - length(float2(d.x + lean, d.y)) / r;
                float body = saturate(core * 1.3 + (n - 0.5) * 1.2);
                return body * smoothstep(1.3, 0.8, y + (n - 0.5) * 0.5) * smoothstep(0.0, 0.05, y);
            }

            float3 March(float3 ro, float3 rd, float2 seed)
            {
                float3 col = 0;
                for (int j = 0; j < _JetCount; j++)
                {
                    float4 jet = _Jet[j];
                    if (jet.z < 0.05) continue;
                    // March only through this jet's bounding box.
                    float3 bmin = float3(jet.x - jet.w * 2.2, 0.0, jet.y - jet.w * 2.2);
                    float3 bmax = float3(jet.x + jet.w * 2.2, jet.z * 1.35, jet.y + jet.w * 2.2);
                    float3 t0 = (bmin - ro) / rd, t1 = (bmax - ro) / rd;
                    float3 tn = min(t0, t1), tf = max(t0, t1);
                    float a = max(max(tn.x, tn.y), max(tn.z, 0.0)), b = min(min(tf.x, tf.y), tf.z);
                    if (rd.y < 0.0) b = min(b, -ro.y / rd.y);       // the floor
                    if (b <= a) continue;
                    const int Steps = 26;
                    float dt = (b - a) / Steps;
                    float t = a + dt * frac(sin(dot(seed, float2(12.9898, 78.233))) * 43758.5453);
                    float trans = 1.0;
                    for (int k = 0; k < Steps; k++)
                    {
                        float3 p = ro + rd * t;
                        float temp = Temp(p, jet);
                        if (temp > 0.002)
                        {
                            float3 e = Ramp(temp) * temp;
                            col += e * trans * dt * 1.5;
                            trans *= exp(-temp * dt * 0.35);     // the flame's own body hides what's behind it a little
                        }
                        t += dt;
                    }
                }
                return col;
            }

            float4 frag(FsV i) : SV_Target
            {
                float3 ro = _WorldSpaceCameraPos;
                float3 rd = normalize(i.rd);
                float3 col = March(ro, rd, i.ndc);
                // The wet stage floor mirrors the flames, broken up a little.
                if (rd.y < 0.0)
                {
                    float tf = -ro.y / rd.y;
                    float3 fp = ro + rd * tf;
                    float3 rr = reflect(rd, normalize(float3(sin(fp.x * 3.1) * 0.02, 1.0, sin(fp.z * 2.3) * 0.02)));
                    col += March(fp + rr * 0.01, rr, i.ndc + 7.0) * 0.4 * exp(-tf * 0.01);
                }
                return float4(col * _Heat, 0.0);
            }
            ENDHLSL
        }
    }
}
