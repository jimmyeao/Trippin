// A painted far backdrop for the HQ shows: a keyed photographic texture
// (Alice-generated, e.g. a city skyline) on a big quad on the horizon.
// Bright pixels (windows, signs) are lifted into HDR so they bloom; they
// brighten with _Lit (the music's energy) but the buildings never move.
// Haze (_Haze, toward _HazeCol) sinks it into the distance.
//  _Mirror   1: its reflection in water: flipped by the quad, stretched
//            and broken into ripples, darker, with a few highlights
//            smeared vertically the way lights reflect on a harbour
Shader "Trippin/HqBackdrop"
{
    Properties
    {
        _MainTex ("Texture", 2D) = "black" {}
        _Gain ("Gain", Float) = 1
        _Lit ("Lights", Float) = 1
        _Haze ("Haze", Float) = 0.2
        _HazeCol ("Haze colour", Color) = (0.12, 0.05, 0.1, 1)
        _Mirror ("Mirror", Float) = 0
        _URange ("U range", Float) = 1
    }
    SubShader
    {
        Tags { "RenderType"="Transparent" "Queue"="Transparent-30" "RenderPipeline"="UniversalPipeline" }
        Blend SrcAlpha OneMinusSrcAlpha
        ZWrite Off
        Cull Off
        Pass
        {
            HLSLPROGRAM
            #pragma vertex vert
            #pragma fragment frag
            #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/Core.hlsl"

            TEXTURE2D(_MainTex);
            SAMPLER(sampler_MainTex);
            CBUFFER_START(UnityPerMaterial)
            float _Gain, _Lit, _Haze, _Mirror, _URange;
            float4 _HazeCol;
            CBUFFER_END

            struct A { float4 pos : POSITION; float2 uv : TEXCOORD0; };
            struct V { float4 pos : SV_POSITION; float2 uv : TEXCOORD0; };
            V vert(A i) { V o; o.pos = TransformObjectToHClip(i.pos.xyz); o.uv = i.uv; return o; }

            float3 Lift(float3 c)
            {
                float l = dot(c, float3(0.299, 0.587, 0.114));
                // Windows and signs: the brightest pixels glow past 1.
                return c * (0.55 + _Lit * 3.2 * smoothstep(0.45, 0.95, l) * l);
            }

            float4 frag(V i) : SV_Target
            {
                float2 uv = float2(i.uv.x * _URange, i.uv.y);
                if (_Mirror > 0.5)
                {
                    // Reflection: uv.y runs 0 at the waterline to 1 at the bottom.
                    float d = uv.y;
                    float rip = sin(d * 180.0 - _Time.y * 2.0) * 0.6 + sin(d * 77.0 + uv.x * 40.0 + _Time.y * 1.3) * 0.4;
                    uv.x += rip * 0.0025 * (0.2 + d);
                    float2 tuv = float2(uv.x, d);   // the reflection of the texture's v = d
                    float4 t = 0;
                    // Smear the lights down the water.
                    for (int k = 0; k < 4; k++)
                        t += SAMPLE_TEXTURE2D(_MainTex, sampler_MainTex, tuv - float2(0, k * 0.012 * (0.3 + d)));
                    t *= 0.25;
                    float3 c = Lift(t.rgb) * 0.3 * (1.0 - d) * (1.0 - d);
                    return float4(c * _Gain, t.a * saturate(1.0 - d * 1.2) * 0.9);
                }
                float4 t = SAMPLE_TEXTURE2D(_MainTex, sampler_MainTex, uv);
                float3 c = lerp(Lift(t.rgb), _HazeCol.rgb, _Haze);
                return float4(c * _Gain, t.a);
            }
            ENDHLSL
        }
    }
}
