// unity_lasers: the surface a fast-scanning laser seems to draw in smoke, a
// flat sheet of light (a scanned fan, "liquid sky") or a cone (a tunnel).
// LasersShow builds the mesh each frame between each projector's beam
// directions (vertex colour = the beam's colour and strength; uv.y the
// distance from the projector in m). Like the beams, it shows only where
// the smoke is (LaserSmoke.hlsl). The scan spreads the beam's light over
// the arc it sweeps, so the sheet dims with distance from the projector,
// and seen edge-on it reads as a bright blade (more smoke along the view).
Shader "Trippin/HqLaserSheet"
{
    Properties
    {
        _Gain ("Gain", Float) = 1
    }
    SubShader
    {
        Tags { "RenderType"="Transparent" "Queue"="Transparent" "RenderPipeline"="UniversalPipeline" }
        Blend One One
        ZWrite Off
        Cull Off
        Pass
        {
            HLSLPROGRAM
            #pragma vertex vert
            #pragma fragment frag
            #pragma target 4.5
            #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/Core.hlsl"
            #include "TrippinCommon.hlsl"
            #include "LaserSmoke.hlsl"

            float _Gain;

            struct A { float3 pos : POSITION; float4 col : COLOR; float2 uv : TEXCOORD0; };
            struct V { float4 pos : SV_POSITION; float4 col : COLOR; float2 uv : TEXCOORD0; float3 wp : TEXCOORD1; };

            V vert(A a)
            {
                V o;
                o.wp = a.pos;
                o.pos = TransformWorldToHClip(a.pos);
                o.col = a.col;
                o.uv = a.uv;
                return o;
            }

            float4 frag(V i) : SV_Target
            {
                float3 toCam = _WorldSpaceCameraPos - i.wp;
                float dc = length(toCam);
                // Seen edge-on, a sheet is a bright blade; face-on, a faint veil.
                float3 n = normalize(cross(ddx(i.wp), ddy(i.wp)) + 1e-6);
                float edgeOn = min(1.0 / (abs(dot(n, toCam / dc)) + 0.15), 4.0);
                // Fog-machine haze fills the room evenly enough that a scanned plane reads as one
                // surface; the banks only brighten it.
                float smoke = 0.35 + 1.3 * saturate(LSDensity(i.wp) * 1.6);
                float spread = 1.0 / (1.0 + 0.22 * i.uv.y);
                // Fade near the lens (a sheet through the camera would fill the frame) and at the far end.
                float fade = smoothstep(1.5, 5.0, dc) * (1.0 - smoothstep(26.0, 38.0, i.uv.y)) * smoothstep(0.0, 0.6, i.uv.y);
                return float4(i.col.rgb * i.col.a * smoke * spread * edgeOn * fade * _Gain, 0);
            }
            ENDHLSL
        }
    }
}
