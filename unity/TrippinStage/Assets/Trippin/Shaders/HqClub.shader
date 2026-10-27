// unity_decks' room: the club behind the booth, already out of focus (it is
// metres behind gear seen from centimetres). Haze beams fanning down from a
// truss in two palette colours, swinging with _Swing; a far LED wall broken
// into soft bokeh discs that follow the spectrum; a dark floor. Drawn first,
// ZWrite off, so the gear and the depth of field go over it.
Shader "Trippin/HqClub"
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

            float _Gain, _Swing, _T, _Hue, _Beams, _Wall;

            float CHash(float2 p) { return frac(sin(dot(p, float2(127.1, 311.7))) * 43758.5453); }

            float4 frag(FsV i) : SV_Target
            {
                float3 rd = normalize(i.rd);
                // Azimuth/elevation of the view ray: the room wraps round the booth.
                float az = atan2(rd.x, rd.z);
                float el = asin(clamp(rd.y, -1.0, 1.0));
                float3 col = float3(0.004, 0.004, 0.007) + float3(0.01, 0.006, 0.014) * saturate(1.0 - abs(el) * 2.0);
                float3 p1 = TPalette(_Hue); p1 *= p1;
                float3 p2 = TPalette(_Hue + 0.37); p2 *= p2;
                // Beams: 8 lamps on a truss above the dance floor, each a soft cone
                // fanning down; seen from the booth they are wide streaks of haze.
                float beams = 0.0;
                float3 bc = 0;
                [unroll] for (int k = 0; k < 8; k++)
                {
                    float x0 = (k - 3.5) * 0.32;
                    float sw = sin(_Swing + k * 0.9) * 0.35 * (k % 2 == 0 ? 1.0 : -1.0);
                    // The beam's line on the "sky" from (x0, 0.55) down toward (x0 + sw, -0.2).
                    float2 a = float2(x0, 0.55), b = float2(x0 + sw, -0.25);
                    float2 q = float2(az, el);
                    float2 ab = b - a;
                    float h = saturate(dot(q - a, ab) / dot(ab, ab));
                    float d = length(q - (a + ab * h));
                    float w = 0.02 + 0.09 * h;
                    float s = exp(-d * d / (w * w)) * (0.35 + 0.65 * h) * (1.0 - h * 0.6);
                    // Haze breaks the beam up a little.
                    s *= 0.75 + 0.25 * sin(h * 9.0 + _T * 0.7 + k * 2.0);
                    bc += (k % 2 == 0 ? p1 : p2) * s;
                    // The lamp itself: a soft hot disc on the truss.
                    float lamp = exp(-dot(q - a, q - a) / 0.0009);
                    bc += lerp(k % 2 == 0 ? p1 : p2, float3(1, 1, 1), 0.5) * lamp * 2.0;
                }
                col += bc * 0.16 * _Beams;
                // Lights far across the club, thrown out of focus: soft discs of
                // different sizes scattered over the far wall and truss, each
                // following a band of the spectrum.
                float2 q2 = float2(az, el);
                [loop] for (int b = 0; b < 36; b++)
                {
                    float hx = CHash(float2(b, 1.7)), hy = CHash(float2(b, 4.1)), hs = CHash(float2(b, 9.3));
                    float2 c = float2(lerp(-1.5, 1.5, hx), lerp(-0.08, 0.5, hy * hy));
                    float r = lerp(0.018, 0.06, hs);
                    float d = length(q2 - c);
                    if (d > r * 1.3) continue;
                    float disc = 1.0 - smoothstep(r * 0.82, r, d);
                    disc *= 0.75 + 0.25 * smoothstep(r * 0.3, r, d);      // brighter rim, like a real lens
                    float sp = RxSpec(hx);
                    float lv = 0.25 + 0.75 * sp;
                    float3 lc = lerp(p1, p2, CHash(float2(b, 2.2)));
                    lc = lerp(lc, float3(1.0, 0.85, 0.7), step(0.8, CHash(float2(b, 5.5))) * 0.6);
                    col += lc * disc * lv * 0.09 * _Wall / (0.4 + hs);
                }
                // A dark floor reflecting a little of the beams.
                if (el < -0.12) col = col * 0.25 + bc * 0.02;
                return float4(col * _Gain, 1);
            }
            ENDHLSL
        }
    }
}
