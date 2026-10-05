// SPIKE pack show, compiled OUTSIDE the Unity project against the engine's built
// Assembly-CSharp.dll (see unity/pack_spike/build.sh). Eight orbs from the pack's
// AssetBundle on a ring: shape = each orb swells with its slice of the eased
// spectrum and the kick; motion = the ring turns on the energy clock and the orbs
// bob on the beat (Rx.beatS).
// Windows step 6: an inner ring of 16 cubes drawn with Graphics.RenderMeshInstanced
// and the bundle's PackInstMat (instancing on the asset), and a lower control ring
// with PackCtlMat (instancing off on the asset).
using UnityEngine;

namespace TrippinPack
{
    public sealed class PackPulse : TrippinStage.KitShow
    {
        const int N = 8, M = 16;
        readonly Transform[] _orb = new Transform[N];
        readonly Matrix4x4[] _inst = new Matrix4x4[M], _ctl = new Matrix4x4[M];
        Mesh _cube;
        RenderParams _instRp, _ctlRp;
        bool _haveInst, _ctlTried;

        protected override void Build()
        {
            var prefab = pack != null ? pack.LoadAsset<GameObject>("PackPulseOrb") : null;
            if (prefab == null) { Debug.LogError("[PackPulse] PackPulseOrb not in the pack bundle"); return; }
            for (int i = 0; i < N; i++) _orb[i] = Object.Instantiate(prefab, transform).transform;
            Debug.Log("[PackPulse] built " + N + " orbs from the pack bundle");

            var im = pack.LoadAsset<Material>("PackInstMat");
            var cm = pack.LoadAsset<Material>("PackCtlMat");
            _cube = Resources.GetBuiltinResource<Mesh>("Cube.fbx");
            if (im == null || cm == null || _cube == null) { Debug.LogError("[PackPulse] instancing assets missing"); return; }
            var big = new Bounds(Vector3.zero, Vector3.one * 200f);
            _instRp = new RenderParams(im) { worldBounds = big };
            _ctlRp = new RenderParams(cm) { worldBounds = big };
            _haveInst = true;
            Debug.Log($"[PackPulse] instancing: PackInstMat.enableInstancing={im.enableInstancing} shader={im.shader.name} supported={im.shader.isSupported}; PackCtlMat.enableInstancing={cm.enableInstancing}");
        }

        protected override void Frame(TrippinStage.ShowState s, float dt)
        {
            if (_orb[0] == null) return;
            for (int i = 0; i < N; i++)
            {
                float a = i / (float)N * Mathf.PI * 2f + rx.clk * 0.05f;
                float bob = 0.8f * Mathf.Abs(Mathf.Sin(rx.beatS * Mathf.PI + i * 0.4f));
                _orb[i].localPosition = new Vector3(Mathf.Cos(a) * 7f, 5f + bob, Mathf.Sin(a) * 7f);
                _orb[i].localScale = Vector3.one * (1.2f + 2.4f * rx.Spec(i / (N - 1f)) + 0.8f * rx.kick);
            }
            if (_haveInst)
            {
                for (int i = 0; i < M; i++)
                {
                    float a = i / (float)M * Mathf.PI * 2f - rx.clk * 0.08f;
                    float h = 0.6f + 2.5f * rx.Spec(i / (M - 1f));
                    var rot = Quaternion.Euler(0f, -a * Mathf.Rad2Deg, 0f);
                    _inst[i] = Matrix4x4.TRS(new Vector3(Mathf.Cos(a) * 3.5f, 5f, Mathf.Sin(a) * 3.5f), rot, new Vector3(0.6f, h, 0.6f));
                    _ctl[i] = Matrix4x4.TRS(new Vector3(Mathf.Cos(a) * 3.5f, 1.2f, Mathf.Sin(a) * 3.5f), rot, new Vector3(0.6f, 0.6f, 0.6f));
                }
                Graphics.RenderMeshInstanced(_instRp, _cube, 0, _inst);
                // The control throws (Unity checks the material's flag), so try it once.
                if (!_ctlTried)
                {
                    _ctlTried = true;
                    try { Graphics.RenderMeshInstanced(_ctlRp, _cube, 0, _ctl); Debug.Log("[PackPulse] control (instancing off) drew without error"); }
                    catch (System.Exception e) { Debug.Log("[PackPulse] control (instancing off) refused: " + e.Message); }
                }
            }
            rig.Orbit(cam, rx, 20f, 9f, 5f, dt);
        }
    }
}
