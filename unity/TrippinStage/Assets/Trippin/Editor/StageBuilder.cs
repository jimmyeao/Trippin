// Builds the stage scene and the Windows player from code, so the whole
// project is reproducible from the CLI:
//   unity run . -- -executeMethod TrippinStage.EditorTools.StageBuilder.BuildPlayer
// (or the "Trippin" menu in the Editor).

using System.IO;
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

            // Output texture sent to Trippin (its external frame is 1920x1080).
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

            // Engine: the link, the show switcher and the frame exporter. Each
            // show is a child named after its Trippin scene.
            var engine = new GameObject("Engine");
            engine.AddComponent<TrippinLink>();
            var mgr = engine.AddComponent<ShowManager>();
            mgr.cam = cam;
            mgr.output = rt;

            var stage = new GameObject("unity_stage");
            stage.transform.SetParent(engine.transform, false);
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
            dir.confettiMat = Mat("Trippin/Confetti", "Confetti");
            dir.phoneMat = Mat("Trippin/Phones", "Phones");
            dir.phoneMat.enableInstancing = true;
            dir.sunMat = Mat("Trippin/Sun", "Sun");
            dir.crowdMeshMat = Mat("Trippin/CrowdMesh", "CrowdMesh");
            dir.crowdMeshMat.enableInstancing = true;

            var crystals = new GameObject("unity_crystals");
            crystals.transform.SetParent(engine.transform, false);
            var cs = crystals.AddComponent<CrystalShow>();
            cs.cam = cam;
            cs.chromeMat = Mat("Trippin/Chrome", "Chrome");
            cs.chromeMat.enableInstancing = true;
            cs.sunMat = dir.sunMat;

            var flowGo = new GameObject("unity_flow");
            flowGo.transform.SetParent(engine.transform, false);
            var fs = flowGo.AddComponent<FlowShow>();
            fs.cam = cam;
            fs.flow = AssetDatabase.LoadAssetAtPath<ComputeShader>($"{Root}/Shaders/Flow.compute");
            if (fs.flow == null) throw new System.Exception("Flow.compute not found");
            fs.pointsMat = Mat("Trippin/Points", "Points");

            var levGo = new GameObject("unity_leviathan");
            levGo.transform.SetParent(engine.transform, false);
            var lv = levGo.AddComponent<LeviathanShow>();
            lv.cam = cam;
            lv.sim = AssetDatabase.LoadAssetAtPath<ComputeShader>($"{Root}/Shaders/Leviathan.compute");
            if (lv.sim == null) throw new System.Exception("Leviathan.compute not found");
            lv.filamentMat = Mat("Trippin/Filament", "Filament");
            lv.pointsMat = fs.pointsMat;
            lv.skinMat = Mat("Trippin/LevSkin", "LevSkin");

            var sculptGo = new GameObject("unity_sculpture");
            sculptGo.transform.SetParent(engine.transform, false);
            var sc = sculptGo.AddComponent<SculptureShow>();
            sc.cam = cam;
            sc.sculptMat = Mat("Trippin/Sculpture", "Sculpture");

            var colGo = new GameObject("unity_colossus");
            colGo.transform.SetParent(engine.transform, false);
            var co = colGo.AddComponent<ColossusShow>();
            co.cam = cam;
            co.androidMat = Mat("Trippin/Android", "Android");
            co.headMat = Mat("Trippin/AndroidHead", "AndroidHead");
            co.cityMat = Mat("Trippin/City", "City");
            // On the asset, at build time: otherwise the build strips the
            // shader's instancing variant and instanced draws show nothing.
            co.cityMat.enableInstancing = true;
            co.skyMat = Mat("Trippin/Sky", "Sky");
            co.groundMat = Mat("Trippin/Ground", "Ground");
            co.beamMat = dir.beamMat;
            co.glowMat = Mat("Trippin/Backglow", "Backglow");

            var stormGo = new GameObject("unity_lightstorm");
            stormGo.transform.SetParent(engine.transform, false);
            var ls = stormGo.AddComponent<LightstormShow>();
            ls.cam = cam;
            ls.beamMat = dir.beamMat;
            ls.glowMat = co.glowMat;
            ls.groundMat = co.groundMat;
            ls.hazeMat = dir.hazeMat;

            mgr.shows = new[] { stage, crystals, flowGo, levGo, sculptGo, colGo, stormGo };
            mgr.names = new[] { "unity_stage", "unity_crystals", "unity_flow", "unity_leviathan", "unity_sculpture", "unity_colossus", "unity_lightstorm" };

            // Frames go to Trippin through the shared-memory file it passes
            // (FrameExporter) — no Spout/Syphon, the same on every platform.

            EditorSceneManager.SaveScene(scene, ScenePath);
            EditorBuildSettings.scenes = new[] { new EditorBuildSettingsScene(ScenePath, true) };
            AssetDatabase.SaveAssets();
            Debug.Log("[StageBuilder] scene built: " + ScenePath);
        }

        // `-stageOut <dir>` builds elsewhere (so a running player isn't
        // locked); `-stageMac` builds the macOS .app (needs Mac build support).
        static string OutDir(string def)
        {
            var a = System.Environment.GetCommandLineArgs();
            for (int i = 0; i + 1 < a.Length; i++)
                if (a[i] == "-stageOut") return a[i + 1];
            return def;
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
                PlayerSettings.allowUnsafeCode = true; // FrameExporter's memcpy
                var mac = System.Array.IndexOf(System.Environment.GetCommandLineArgs(), "-stageMac") >= 0;
                var report = BuildPipeline.BuildPlayer(new BuildPlayerOptions
                {
                    scenes = new[] { ScenePath },
                    locationPathName = mac ? OutDir("BuildMac") + "/TrippinStage.app" : OutDir("Build") + "/TrippinStage.exe",
                    target = mac ? BuildTarget.StandaloneOSX : BuildTarget.StandaloneWindows64,
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
