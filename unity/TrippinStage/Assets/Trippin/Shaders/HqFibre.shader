// unity_fibres: a lawn of fibre-optic strands, drawn procedurally
// (Graphics.RenderPrimitives, SV_VertexID): each strand is a camera-facing
// ribbon of SEG segments bending in the wind with a glowing tip. The roots
// sit on a jittered grid over the field (_Field: x half-width, z near, z far;
// _Cols x _Rows strands). Bending: a slow sway plus the gusts (_Gust[i]: front position
// along _GustDir, amplitude, width) that roll across the field, and the
// kick rings (_Ring[i]: radius, amplitude) that push the strands out from
// the centre. Tips glow with the spectrum (bass in the middle of the field,
// highs at the sides) in the palette; a gust or ring brightens what it
// passes. Additive and depth-free: the show runs no depth of field.
Shader "Trippin/HqFibre"
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

            #define SEG 6
            #define VPS (SEG * 6 + 6)
            float _Gain, _T, _Hue, _Width, _Tip, _Cols, _Rows;
            float4 _Field;
            float4 _GustDir;
            float4 _Gust[4], _Ring[4];

            uint Pcg(uint v) { uint s = v * 747796405u + 2891336453u; uint w = ((s >> ((s >> 28u) + 4u)) ^ s) * 277803737u; return (w >> 22u) ^ w; }
            float Rnd(uint a, uint salt) { return Pcg(a * 9781u + salt * 6271u) / 4294967295.0; }

            struct V { float4 pos : SV_POSITION; float2 uv : TEXCOORD0; float3 col : TEXCOORD1; float tip : TEXCOORD2; };

            V vert(uint vid : SV_VertexID)
            {
                V o;
                uint id = vid / VPS, k = vid % VPS;
                // Root: a jittered cell of the field.
                uint cols = (uint)_Cols;
                float2 cell = float2(id % cols, id / cols);
                float2 size = float2(_Field.x * 2.0, _Field.z - _Field.y);
                float2 root = float2(-_Field.x, _Field.y) + (cell + float2(Rnd(id, 1), Rnd(id, 2))) * size / float2(_Cols, _Rows);
                float len = 0.5 + 0.6 * Rnd(id, 3);
                float ph = Rnd(id, 4) * 6.2832;
                // Wind: a slow sway, the gusts rolling across, the kick rings pushing out.
                float2 bend = float2(sin(_T * 0.9 + ph + root.x * 0.3), cos(_T * 0.7 + ph * 1.3 + root.y * 0.2)) * 0.05;
                float lift = 0.0;
                float along = dot(root, _GustDir.xy);
                [unroll] for (int g = 0; g < 4; g++)
                {
                    float d = (along - _Gust[g].x) / max(_Gust[g].z, 0.1);
                    float e = exp(-d * d) * _Gust[g].y;
                    bend += _GustDir.xy * e;
                    lift += e;
                }
                float r = length(root - float2(0.0, 8.0));
                float2 out2 = (root - float2(0.0, 8.0)) / max(r, 1e-3);
                [unroll] for (int q = 0; q < 4; q++)
                {
                    float d = r - _Ring[q].x;
                    float e = exp(-d * d * 0.5) * _Ring[q].y;
                    bend += out2 * e;
                    lift += e;
                }
                bend = clamp(bend, -0.85, 0.85) * len;
                // Spectrum: bass in the middle of the field, highs out at the sides.
                float sx = saturate(abs(root.x) / _Field.x);
                float lvl = RxSpec(sx * 0.9 + 0.03);
                float3 col = TPalette(_Hue + sx * 0.35 + Rnd(id, 5) * 0.08);
                col *= col;
                // Fade out toward the field's edges so it has no hard border.
                float edge = smoothstep(0.0, 5.0, _Field.x - abs(root.x)) * smoothstep(0.0, 3.0, root.y - _Field.y) * smoothstep(0.0, 10.0, _Field.z - root.y);
                float bright = (0.06 + 2.2 * lvl * lvl + 3.0 * saturate(lift)) * (0.4 + 1.2 * Rnd(id, 6) * Rnd(id, 7)) * edge;

                float3 camR = UNITY_MATRIX_V[0].xyz, camU = UNITY_MATRIX_V[1].xyz;
                float3 wp;
                if (k < SEG * 6)
                {
                    uint seg = k / 6, c = k % 6;
                    uint endA = (c == 1 || c == 4 || c == 5) ? 1u : 0u;
                    float side = (c == 2 || c == 3 || c == 5) ? 1.0 : -1.0;
                    float s = (seg + endA) / (float)SEG;
                    float b2 = s * s;
                    float3 p = float3(root.x + bend.x * b2, len * s * (1.0 - 0.35 * dot(bend, bend) / (len * len) * b2), root.y + bend.y * b2);
                    float3 t = normalize(float3(bend.x * 2.0 * s, len, bend.y * 2.0 * s));
                    float3 view = normalize(_WorldSpaceCameraPos - p);
                    float3 w = normalize(cross(t, view) + 1e-5);
                    wp = p + w * side * _Width;
                    o.uv = float2(side, s);
                    o.tip = 0.0;
                    o.col = col * bright * (0.04 + 0.08 * s);
                }
                else
                {
                    uint c = k - SEG * 6;
                    float2 q = float2((c == 1 || c == 4 || c == 5) ? 1.0 : -1.0, (c == 2 || c == 3 || c == 5) ? 1.0 : -1.0);
                    float3 tipP = float3(root.x + bend.x, len * (1.0 - 0.35 * dot(bend, bend) / (len * len)), root.y + bend.y);
                    wp = tipP + (camR * q.x + camU * q.y) * _Tip;
                    o.uv = q;
                    o.tip = 1.0;
                    o.col = col * bright;
                }
                o.pos = TransformWorldToHClip(wp);
                return o;
            }

            float4 frag(V i) : SV_Target
            {
                float a;
                if (i.tip > 0.5)
                {
                    float d2 = dot(i.uv, i.uv);
                    a = exp(-d2 * 9.0) * 1.6 + exp(-d2 * 2.5) * 0.25;
                }
                else a = 1.0 - abs(i.uv.x);
                return float4(i.col * a * _Gain, 0);
            }
            ENDHLSL
        }
    }
}
