// Builds the stage scene and the Windows player from code, so the whole
// project is reproducible from the CLI:
//   unity run . -- -executeMethod TrippinStage.EditorTools.StageBuilder.BuildPlayer
// (or the "Trippin" menu in the Editor).

using System.IO;
using Klak.Spout;
using UnityEditor;
using UnityEditor.Build.Reporting;
using UnityEditor.SceneManagement;
using UnityEngine;
using UnityEngine.Rendering;
using UnityEngine.Rendering.Universal;

namespace TrippinStage.EditorTools
{
    public static class StageBuilder
    {
        const string Root = "Assets/Trippin";
        const string ScenePath = "Assets/Scenes/Stage.unity";
        public const string SenderName = "Trippin Stage";

        static Material Mat(string shader, string name)
        {
            Directory.CreateDirectory($"{Root}/Materials");
            var path = $"{Root}/Materials/{name}.mat";
            var sh = Shader.Find(shader);
            if (sh == null) throw new System.Exception($"shader {shader} not found (compile error?)");
            var m = AssetDatabase.LoadAssetAtPath<Material>(path);
            if (m == null)
            {
                m = new Material(sh);
                AssetDatabase.CreateAsset(m, path);
            }
            else m.shader = sh;
            return m;
        }

        [MenuItem("Trippin/Build Stage Scene")]
        public static void BuildScene()
        {
            Directory.CreateDirectory("Assets/Scenes");
            Directory.CreateDirectory($"{Root}/Generated");
            var scene = EditorSceneManager.NewScene(NewSceneSetup.EmptyScene, NewSceneMode.Single);

            // Output texture Spout sends (Trippin's external frame is 1920x1080).
            var rtPath = $"{Root}/Generated/StageOutput.renderTexture";
            AssetDatabase.DeleteAsset(rtPath);
            var rt = new RenderTexture(1920, 1080, 24, RenderTextureFormat.ARGB32, RenderTextureReadWrite.sRGB)
            {
                name = "StageOutput",
                antiAliasing = 4,
            };
            AssetDatabase.CreateAsset(rt, rtPath);

            var camGo = new GameObject("Stage Camera");
            var cam = camGo.AddComponent<Camera>();
            cam.clearFlags = CameraClearFlags.SolidColor;
            cam.backgroundColor = Color.black;
            cam.fieldOfView = 52;
            cam.nearClipPlane = 0.1f;
            cam.farClipPlane = 600f;
            cam.targetTexture = rt;
            var acd = camGo.AddComponent<UniversalAdditionalCameraData>();
            acd.renderPostProcessing = true;
            acd.antialiasing = AntialiasingMode.SubpixelMorphologicalAntiAliasing;

            // The window itself only shows a preview (StageDirector.OnGUI);
            // this camera just clears it.
            var disp = new GameObject("Display Camera").AddComponent<Camera>();
            disp.clearFlags = CameraClearFlags.SolidColor;
            disp.backgroundColor = Color.black;
            disp.cullingMask = 0;
            disp.depth = 10;

            // Post: bloom carries the beams and LEDs; ACES like Trippin.
            var profPath = $"{Root}/Generated/StagePost.asset";
            AssetDatabase.DeleteAsset(profPath);
            var prof = ScriptableObject.CreateInstance<VolumeProfile>();
            AssetDatabase.CreateAsset(prof, profPath);
            var bloom = prof.Add<Bloom>(true);
            bloom.intensity.Override(1.6f);
            bloom.threshold.Override(0.9f);
            bloom.scatter.Override(0.78f);
            var tm = prof.Add<Tonemapping>(true);
            tm.mode.Override(TonemappingMode.ACES);
            var vig = prof.Add<Vignette>(true);
            vig.intensity.Override(0.25f);
            foreach (var c in prof.components) AssetDatabase.AddObjectToAsset(c, prof);
            EditorUtility.SetDirty(prof);
            var vol = new GameObject("Post").AddComponent<Volume>();
            vol.isGlobal = true;
            vol.sharedProfile = prof;

            var stage = new GameObject("Stage");
            stage.AddComponent<TrippinLink>();
            var dir = stage.AddComponent<StageDirector>();
            dir.cam = cam;
            dir.output = rt;
            dir.beamMat = Mat("Trippin/Beam", "Beam");
            dir.ledMat = Mat("Trippin/LedWall", "LedWall");
            dir.structMat = Mat("Trippin/Structure", "Structure");
            dir.crowdMat = Mat("Trippin/Crowd", "Crowd");
            dir.crowdMat.enableInstancing = true;
            dir.hazeMat = Mat("Trippin/Haze", "Haze");
            dir.addMat = Mat("Trippin/Additive", "Additive");

            var sender = stage.AddComponent<SpoutSender>();
            sender.spoutName = SenderName;
            sender.captureMethod = CaptureMethod.Texture;
            sender.sourceTexture = rt;
            sender.keepAlpha = false;
            var res = AssetDatabase.LoadAssetAtPath<SpoutResources>("Packages/jp.keijiro.klak.spout/Editor/SpoutResources.asset");
            if (res == null) throw new System.Exception("KlakSpout resources not found");
            sender.SetResources(res);

            EditorSceneManager.SaveScene(scene, ScenePath);
            EditorBuildSettings.scenes = new[] { new EditorBuildSettingsScene(ScenePath, true) };
            AssetDatabase.SaveAssets();
            Debug.Log("[StageBuilder] scene built: " + ScenePath);
        }

        // `-stageOut <dir>` builds elsewhere (so a running player isn't locked).
        static string OutPath()
        {
            var a = System.Environment.GetCommandLineArgs();
            for (int i = 0; i + 1 < a.Length; i++)
                if (a[i] == "-stageOut") return a[i + 1] + "/TrippinStage.exe";
            return "Build/TrippinStage.exe";
        }

        [MenuItem("Trippin/Build Player (Windows)")]
        public static void BuildPlayer()
        {
            int code = 0;
            try
            {
                BuildScene();
                PlayerSettings.productName = "Trippin Stage";
                PlayerSettings.companyName = "Trippin";
                PlayerSettings.runInBackground = true;
                PlayerSettings.visibleInBackground = true;
                PlayerSettings.fullScreenMode = FullScreenMode.Windowed;
                PlayerSettings.defaultScreenWidth = 960;
                PlayerSettings.defaultScreenHeight = 540;
                PlayerSettings.resizableWindow = true;
                PlayerSettings.SplashScreen.show = false;
                // Spout shares D3D11 textures.
                PlayerSettings.SetUseDefaultGraphicsAPIs(BuildTarget.StandaloneWindows64, false);
                PlayerSettings.SetGraphicsAPIs(BuildTarget.StandaloneWindows64,
                    new[] { UnityEngine.Rendering.GraphicsDeviceType.Direct3D11 });
                var report = BuildPipeline.BuildPlayer(new BuildPlayerOptions
                {
                    scenes = new[] { ScenePath },
                    locationPathName = OutPath(),
                    target = BuildTarget.StandaloneWindows64,
                    options = BuildOptions.None,
                });
                Debug.Log($"[StageBuilder] build {report.summary.result}: {report.summary.totalSize / 1048576} MB, {report.summary.totalErrors} errors");
                if (report.summary.result != BuildResult.Succeeded) code = 1;
            }
            catch (System.Exception e)
            {
                Debug.LogError("[StageBuilder] " + e);
                code = 1;
            }
            if (Application.isBatchMode) EditorApplication.Exit(code);
        }
    }
}
