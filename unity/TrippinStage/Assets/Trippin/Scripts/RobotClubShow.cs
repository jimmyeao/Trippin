// unity_robot_club: a festival-style wide shot from the back of a crowd of
// generated robots (six Alice/Hunyuan3D designs: ceramic, chrome, tin toy,
// gold art-deco, yellow mech, neon cyberpunk), all facing the stage where the
// android DJ from unity_djbot plays behind its decks under a laser fan, a halo
// and a wall of LEDs. The DJ stage is DjBotShow's, built deeper in the scene
// and scaled up; the crowd is instanced, so 144 robots cost one draw call per
// design.
//  - Shape / luminance / DJ motion: as unity_djbot (visor spectrum, faders on
//    the bands, routine every 4 bars, arms up on a drop, eased impact).
//  - Crowd motion (CrowdMesh.shader): each robot bounces on the beat with its
//    own lag (calm-aware) and its upper body sways on the energy clock, wider as
//    the track builds; the crowd is lit by the stage colours.
//  - Camera: behind the crowd, drifting slowly, pushing in on a build.
// Needs Resources/Robots/*.bytes (tools/crowd_meshes.py --out Robots --loose);
// without them it is just the DJ stage seen from the back of an empty hall.
using System.Collections.Generic;
using UnityEngine;

namespace TrippinStage
{
    public sealed class RobotClubShow : DjBotShow
    {
        const int Rows = 9, Cols = 18;
        const float StageZ = 36f, StageScale = 1.5f;
        Mesh[] _meshes = new Mesh[0];
        Matrix4x4[][] _by;
        RenderParams _rp;
        bool _crowdReady;

        protected override Transform MakeRoot()
        {
            var g = new GameObject("stage");
            g.transform.SetParent(transform, false);
            g.transform.localPosition = new Vector3(0f, 0f, StageZ);
            g.transform.localScale = Vector3.one * StageScale;
            return g.transform;
        }

        protected override void Build()
        {
            base.Build();
            var list = new List<Mesh>();
            foreach (var t in Resources.LoadAll<TextAsset>("Robots"))
            {
                var m = StageDirector.LoadCrowdMesh(t);
                if (m != null) list.Add(m);
            }
            if (list.Count == 0 || crowdMeshMat == null) return;
            _meshes = list.ToArray();
            var by = new List<Matrix4x4>[_meshes.Length];
            for (int k = 0; k < by.Length; k++) by[k] = new List<Matrix4x4>();
            for (int r = 0; r < Rows; r++)
                for (int c = 0; c < Cols; c++)
                {
                    int n = r * Cols + c;
                    float x = (c - (Cols - 1) * 0.5f) * 2.5f + (Kit.H(n, 11) - 0.5f) * 1.2f;
                    float z = 1.5f + r * 2.6f + (Kit.H(n, 12) - 0.5f) * 1.2f;
                    float h = 3.0f + 1.0f * Kit.H(n, 13);
                    float yaw = (Kit.H(n, 14) - 0.5f) * 40f;
                    by[Mathf.Min((int)(Kit.H(n, 15) * _meshes.Length), _meshes.Length - 1)].Add(
                        Matrix4x4.TRS(new Vector3(x, 0f, z), Quaternion.Euler(0f, yaw, 0f), Vector3.one * h));
                }
            _by = new Matrix4x4[by.Length][];
            for (int k = 0; k < by.Length; k++) _by[k] = by[k].ToArray();
            crowdMeshMat.enableInstancing = true;
            _rp = new RenderParams(crowdMeshMat)
            {
                shadowCastingMode = UnityEngine.Rendering.ShadowCastingMode.Off,
                worldBounds = new Bounds(new Vector3(0f, 2f, 12f), new Vector3(70f, 8f, 40f)),
            };
            _crowdReady = true;
        }

        protected override void Frame(ShowState s, float dt)
        {
            base.Frame(s, dt);
            if (!_crowdReady) return;
            for (int k = 0; k < _meshes.Length; k++)
                if (_by[k].Length > 0) Graphics.RenderMeshInstanced(_rp, _meshes[k], 0, _by[k]);
        }

        protected override void CameraPose(out Vector3 pos, out Vector3 look)
        {
            float sway = Mathf.Sin(rx.phrase * 0.5f);
            pos = new Vector3(sway * 7f, 4.6f + 0.6f * Mathf.Sin(rx.phrase), -6f + 6f * rx.tension);
            look = new Vector3(0f, 9.5f, StageZ);
        }
    }
}
