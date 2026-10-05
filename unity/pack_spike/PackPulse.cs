// SPIKE pack show, compiled OUTSIDE the Unity project against the engine's built
// Assembly-CSharp.dll (see unity/pack_spike/build.sh). Eight orbs from the pack's
// AssetBundle on a ring: shape = each orb swells with its slice of the eased
// spectrum and the kick; motion = the ring turns on the energy clock and the orbs
// bob on the beat (Rx.beatS).
using UnityEngine;

namespace TrippinPack
{
    public sealed class PackPulse : TrippinStage.KitShow
    {
        const int N = 8;
        readonly Transform[] _orb = new Transform[N];

        protected override void Build()
        {
            var prefab = pack != null ? pack.LoadAsset<GameObject>("PackPulseOrb") : null;
            if (prefab == null) { Debug.LogError("[PackPulse] PackPulseOrb not in the pack bundle"); return; }
            for (int i = 0; i < N; i++) _orb[i] = Object.Instantiate(prefab, transform).transform;
            Debug.Log("[PackPulse] built " + N + " orbs from the pack bundle");
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
            rig.Orbit(cam, rx, 20f, 9f, 5f, dt);
        }
    }
}
