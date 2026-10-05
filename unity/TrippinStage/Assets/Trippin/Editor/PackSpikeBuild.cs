// SPIKE: build the pack-loader test AssetBundle for macOS and Windows.
//   Unity -batchmode -quit -projectPath unity/TrippinStage -executeMethod TrippinStage.EditorTools.PackSpikeBuild.Build -packOut <dir>
using System.IO;
using UnityEditor;
using UnityEngine;

namespace TrippinStage.EditorTools
{
    public static class PackSpikeBuild
    {
        public static void Build()
        {
            string outDir = "PackSpikeOut";
            var a = System.Environment.GetCommandLineArgs();
            for (int i = 0; i + 1 < a.Length; i++) if (a[i] == "-packOut") outDir = a[i + 1];
            Directory.CreateDirectory("Assets/PackSpike");
            var sh = Shader.Find("Trippin/Robot");
            if (sh == null) { Debug.LogError("[PackSpike] Trippin/Robot shader not found"); return; }
            var mat = new Material(sh) { name = "PackPulseMat", enableInstancing = true };
            mat.SetFloat("_Emit", 1.2f);
            AssetDatabase.CreateAsset(mat, "Assets/PackSpike/PackPulseMat.mat");
            var go = GameObject.CreatePrimitive(PrimitiveType.Sphere);
            go.name = "PackPulseOrb";
            Object.DestroyImmediate(go.GetComponent<Collider>());
            go.GetComponent<MeshRenderer>().sharedMaterial = mat;
            PrefabUtility.SaveAsPrefabAsset(go, "Assets/PackSpike/PackPulseOrb.prefab");
            Object.DestroyImmediate(go);
            AssetImporter.GetAtPath("Assets/PackSpike/PackPulseOrb.prefab").assetBundleName = "packpulse.bundle";
            // Windows step 6: a bundle-only instancing shader, one material with
            // instancing on the ASSET and a control with it off (the stripping trap).
            File.Copy("Assets/Trippin/Editor/PackInst.shader.txt", "Assets/PackSpike/PackInst.shader", true);
            AssetDatabase.ImportAsset("Assets/PackSpike/PackInst.shader", ImportAssetOptions.ForceUpdate);
            var ish = AssetDatabase.LoadAssetAtPath<Shader>("Assets/PackSpike/PackInst.shader");
            if (ish == null) { Debug.LogError("[PackSpike] PackInst.shader failed to import"); return; }
            foreach (var (n, inst) in new[] { ("PackInstMat", true), ("PackCtlMat", false) })
            {
                var im = new Material(ish) { name = n, enableInstancing = inst };
                im.SetColor("_Tint", inst ? Color.white : new Color(1f, 0.25f, 0.25f));
                AssetDatabase.CreateAsset(im, $"Assets/PackSpike/{n}.mat");
                AssetImporter.GetAtPath($"Assets/PackSpike/{n}.mat").assetBundleName = "packpulse.bundle";
            }
            AssetDatabase.SaveAssets();
            foreach (var (t, sub) in new[] { (BuildTarget.StandaloneOSX, "mac"), (BuildTarget.StandaloneWindows64, "win") })
            {
                string d = Path.Combine(outDir, sub);
                Directory.CreateDirectory(d);
                var m = BuildPipeline.BuildAssetBundles(d, BuildAssetBundleOptions.None, t);
                Debug.Log($"[PackSpike] {t}: {(m != null ? string.Join(",", m.GetAllAssetBundles()) : "FAILED")}");
            }
        }
    }
}
