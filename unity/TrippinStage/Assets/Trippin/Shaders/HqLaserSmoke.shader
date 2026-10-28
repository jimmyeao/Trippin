// unity_lasers' smoke: a full-screen march through the fog-machine smoke
// inside the stage box, from the volume LaserLight.compute bakes each frame
// (light from the beams in rgb, density in a; one fetch a step: sampling the
// noise per step cost 1.45 ms), so the clouds glow in the laser colours where
// the beams cut them and stay near black elsewhere. Stops at
// the scene depth, so the truss and projectors stay in front of what's behind.
Shader "Trippin/HqLaserSmoke"
{
    Properties
    {
        _Gain ("Gain", Float) = 1
    }
    SubShader
    {
        Tags { "RenderType"="Transparent" "Queue"="Transparent-10" "RenderPipeline"="UniversalPipeline" }
        Blend One OneMinusSrcAlpha
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
            #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/DeclareDepthTexture.hlsl"
            #include "TrippinCommon.hlsl"
            #include "TrippinFull.hlsl"

            TEXTURE3D(_Light); SAMPLER(sampler_Light);
            float4 _BoxMin, _BoxSize, _Amb;
            float _Gain, _Sigma, _Frame;

            float4 frag(FsV i) : SV_Target
            {
                float3 ro = _WorldSpaceCameraPos;
                float len = length(i.rd);
                float3 rd = i.rd / len;
                // i.rd has a view-space depth of 1, so eye depth * len is the distance along rd.
                float2 suv = GetNormalizedScreenSpaceUV(i.pos);
                float tmax = LinearEyeDepth(SampleSceneDepth(suv), _ZBufferParams) * len;
                float3 inv = 1.0 / (abs(rd) > 1e-5 ? rd : 1e-5);
                float3 a = (_BoxMin.xyz - ro) * inv, b = (_BoxMin.xyz + _BoxSize.xyz - ro) * inv;
                float3 lo = min(a, b), hi = max(a, b);
                float t0 = max(max(lo.x, lo.y), max(lo.z, 0.0));
                float t1 = min(min(hi.x, hi.y), min(hi.z, tmax));
                if (t1 <= t0) return 0;
                const int N = 24;
                float dt = (t1 - t0) / N;
                float j = frac(52.9829189 * frac(dot(i.pos.xy, float2(0.06711056, 0.00583715)) + _Frame * 0.618034));
                float T = 1.0;
                float3 col = 0;
                [loop] for (int k = 0; k < N; k++)
                {
                    float3 p = ro + rd * (t0 + (k + j) * dt);
                    float tk = t0 + (k + j) * dt;
                    // Thin out near the lens, or a low shot sits inside a glowing bank.
                    float4 v = SAMPLE_TEXTURE3D_LOD(_Light, sampler_Light, (p - _BoxMin.xyz) / _BoxSize.xyz, 0);
                    float d = v.a * smoothstep(1.5, 6.0, tk);
                    if (d < 1e-3) continue;
                    float3 L = min(v.rgb, 3.5) + _Amb.rgb;
                    float s = 1.0 - exp(-d * _Sigma * dt);
                    col += T * s * L;
                    T *= 1.0 - s;
                    if (T < 0.03) break;
                }
                return float4(col * _Gain, (1.0 - T) * _Amb.a);
            }
            ENDHLSL
        }
    }
}
