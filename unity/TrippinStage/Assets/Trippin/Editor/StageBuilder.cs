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

        /// An HQ texture from Assets/Trippin/Textures (Alice-generated art).
        static Texture2D Tex(string file, TextureWrapMode wrap = TextureWrapMode.Clamp)
        {
            var path = $"{Root}/Textures/{file}";
            if (AssetImporter.GetAtPath(path) is TextureImporter ti)
            {
                ti.wrapMode = wrap;
                ti.alphaIsTransparency = true;
                ti.mipmapEnabled = true;
                ti.textureCompression = TextureImporterCompression.CompressedHQ;
                ti.maxTextureSize = 4096;
                ti.SaveAndReimport();
            }
            var t = AssetDatabase.LoadAssetAtPath<Texture2D>(path);
            if (t == null) throw new System.Exception($"texture {path} not found");
            return t;
        }

        /// A gear print (faceplate, label): anisotropic filtering keeps the small
        /// text legible at the grazing angles of the close-ups.
        static Texture2D GearTex(string file)
        {
            var t = Tex(file);
            if (AssetImporter.GetAtPath($"{Root}/Textures/{file}") is TextureImporter ti && ti.anisoLevel != 8)
            {
                ti.anisoLevel = 8;
                ti.SaveAndReimport();
                t = AssetDatabase.LoadAssetAtPath<Texture2D>($"{Root}/Textures/{file}");
            }
            return t;
        }

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
            // Depth of field, off: an HQ show turns it on for itself (HqPost).
            var dof = prof.Add<DepthOfField>(true);
            dof.mode.Override(DepthOfFieldMode.Off);
            dof.highQualitySampling.Override(true);
            foreach (var c in prof.components) AssetDatabase.AddObjectToAsset(c, prof);
            EditorUtility.SetDirty(prof);
            var vol = new GameObject("Post").AddComponent<Volume>();
            vol.isGlobal = true;
            vol.sharedProfile = prof;
            vol.gameObject.AddComponent<HqPostDriver>();

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
            cs.nebulaMat = Mat("Trippin/Nebula", "Nebula");
            cs.pointsMat = Mat("Trippin/Points", "Points");

            var flowGo = new GameObject("unity_flow");
            flowGo.transform.SetParent(engine.transform, false);
            var fs = flowGo.AddComponent<FlowShow>();
            fs.cam = cam;
            fs.flow = AssetDatabase.LoadAssetAtPath<ComputeShader>($"{Root}/Shaders/Flow.compute");
            if (fs.flow == null) throw new System.Exception("Flow.compute not found");
            fs.pointsMat = Mat("Trippin/Points", "Points");
            fs.nebulaMat = Mat("Trippin/Nebula", "Nebula");
            fs.glowMat = Mat("Trippin/Backglow", "Backglow");

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
            sc.skyMat = Mat("Trippin/Sky", "Sky");
            sc.groundMat = Mat("Trippin/Ground", "Ground");
            sc.hazeMat = Mat("Trippin/Haze", "Haze");
            sc.glowMat = Mat("Trippin/Backglow", "Backglow");
            sc.beamMat = dir.beamMat;

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

            var tidalGo = new GameObject("unity_tidal_cathedral");
            tidalGo.transform.SetParent(engine.transform, false);
            var tidal = tidalGo.AddComponent<TidalCathedralShow>();
            tidal.cam = cam;
            tidal.sailMat = Mat("Trippin/TidalSail", "TidalSail");
            tidal.seaMat = Mat("Trippin/TidalSea", "TidalSea");
            tidal.skyMat = Mat("Trippin/TidalSky", "TidalSky");
            tidal.glowMat = sc.glowMat;
            tidal.coreMat = Mat("Trippin/TidalCore", "TidalCore");
            var rose = AssetDatabase.LoadAssetAtPath<Texture2D>($"{Root}/Textures/TidalRose.png");
            if (rose == null) throw new System.Exception("TidalRose.png not found");
            tidal.coreMat.SetTexture("_RoseTex", rose);
            tidal.frameMat = Mat("Trippin/TidalFrame", "TidalFrame");
            tidal.hazeMat = Mat("Trippin/TidalMist", "TidalMist");
            tidal.beamMat = dir.beamMat;

            var stormGo = new GameObject("unity_lightstorm");
            stormGo.transform.SetParent(engine.transform, false);
            var ls = stormGo.AddComponent<LightstormShow>();
            ls.cam = cam;
            ls.beamMat = dir.beamMat;
            ls.glowMat = co.glowMat;
            ls.groundMat = co.groundMat;
            ls.hazeMat = dir.hazeMat;


            var prismGo = new GameObject("unity_prism");
            prismGo.transform.SetParent(engine.transform, false);
            var pr = prismGo.AddComponent<PrismShow>();
            pr.cam = cam;
            pr.beamMat = dir.beamMat;
            pr.groundMat = co.groundMat;
            pr.hazeMat = dir.hazeMat;

            var auroraGo = new GameObject("unity_aurora");
            auroraGo.transform.SetParent(engine.transform, false);
            var au = auroraGo.AddComponent<AuroraShow>();
            au.cam = cam;
            au.auroraMat = Mat("Trippin/Aurora", "Aurora");
            au.beamMat = dir.beamMat;
            au.groundMat = co.groundMat;
            au.hazeMat = dir.hazeMat;

            var bassGo = new GameObject("unity_basscore");
            bassGo.transform.SetParent(engine.transform, false);
            var bc = bassGo.AddComponent<BasscoreShow>();
            bc.cam = cam;
            bc.membraneMat = Mat("Trippin/Membrane", "Membrane");
            bc.beamMat = dir.beamMat;
            bc.groundMat = co.groundMat;
            bc.hazeMat = dir.hazeMat;

            var pillarsGo = new GameObject("unity_pillars");
            pillarsGo.transform.SetParent(engine.transform, false);
            var pl = pillarsGo.AddComponent<PillarsShow>();
            pl.cam = cam;
            pl.beamMat = dir.beamMat;
            pl.groundMat = co.groundMat;
            pl.hazeMat = dir.hazeMat;

            // Kit shows (KitShow subclasses): wired uniformly, one loop for all of them.
            var kitSurface = Mat("Trippin/WireSurface", "WireSurface");
            var kitTube = Mat("Trippin/WireTube", "WireTube");
            var kitOrb = Mat("Trippin/Orb", "Orb");
            var kitMembrane = Mat("Trippin/Membrane", "Membrane");
            var kitRibbon = Mat("Trippin/Tube", "Tube");
            var kitDeep = Mat("Trippin/DeepSpace", "DeepSpace");
            var kitCorridor = Mat("Trippin/Corridor", "Corridor");
            var kitScope = Mat("Trippin/Scope", "Scope");
            var kitLand = Mat("Trippin/Land", "Land");
            var kitHorizon = Mat("Trippin/Horizon", "Horizon");
            var kitRobot = Mat("Trippin/Robot", "Robot");
            var kitScreen = Mat("Trippin/Screen", "Screen");
            var kitShows = new (string name, System.Type type)[]
            {
                ("unity_helix", typeof(HelixShow)),
                ("unity_tesseract", typeof(TesseractShow)),
                ("unity_polyhedra", typeof(PolyhedraShow)),
                ("unity_knot", typeof(KnotShow)),
                ("unity_gyroscope", typeof(GyroscopeShow)),
                ("unity_lattice", typeof(LatticeShow)),
                ("unity_pendulum", typeof(PendulumShow)),
                ("unity_spectrum", typeof(SpectrumShow)),
                ("unity_eclipse", typeof(EclipseShow)),
                ("unity_lightrain", typeof(LightRainShow)),
                ("unity_warp", typeof(WarpShow)),
                ("unity_ledwall", typeof(LedWallShow)),
                ("unity_fountain", typeof(FountainShow)),
                ("unity_galaxy", typeof(GalaxyShow)),
                ("unity_nebula", typeof(NebulaShow)),
                ("unity_ocean", typeof(OceanShow)),
                ("unity_chladni", typeof(ChladniShow)),
                ("unity_tunnel", typeof(TunnelShow)),
                ("unity_orb", typeof(OrbShow)),
                ("unity_radar", typeof(RadarShow)),
                ("unity_djbot", typeof(DjBotShow)),
                ("unity_robot_rave", typeof(RobotRaveShow)),
                ("unity_assembly", typeof(AssemblyShow)),
                ("unity_robot_club", typeof(RobotClubShow)),
                ("unity_robot_band", typeof(RobotBandShow)),
                ("unity_dj_battle", typeof(DjBattleShow)),
                ("unity_highlands", typeof(HighlandsShow)),
                ("unity_coaster", typeof(CoasterShow)),
                ("unity_fluid", typeof(FluidShow)),
                ("unity_murmuration", typeof(MurmurationShow)),
                ("unity_ribbons", typeof(RibbonsShow)),
                ("unity_bloom", typeof(BloomShow)),
                ("unity_marble", typeof(MarbleShow)),
                ("unity_fireworks", typeof(FireworksShow)),
                ("unity_chrome", typeof(ChromeShow)),
                ("unity_kaleido", typeof(KaleidoShow)),
                ("unity_confetti", typeof(ConfettiShow)),
                ("unity_rain", typeof(RainShow)),
                ("unity_pool", typeof(PoolShow)),
                ("unity_synthwave", typeof(SynthwaveShow)),
                ("unity_neon_portals", typeof(NeonPortalsShow)),
                ("unity_fire", typeof(FireShow)),
                ("unity_ink", typeof(InkShow)),
                ("unity_fireflies", typeof(FirefliesShow)),
                ("unity_winter", typeof(WinterShow)),
                ("unity_bubbles", typeof(BubblesShow)),
                ("unity_halloween", typeof(HalloweenShow)),
                ("unity_meteors", typeof(MeteorsShow)),
                ("unity_lava", typeof(LavaShow)),
                ("unity_jellyfish", typeof(JellyfishShow)),
                ("unity_decks", typeof(DecksShow)),
                ("unity_lasers", typeof(LasersShow)),
                ("unity_gold", typeof(GoldShow)),
                ("unity_controller", typeof(ControllerShow)),
                ("unity_groovebox", typeof(GrooveboxShow)),
                ("unity_movingheads", typeof(MovingHeadsShow)),
                ("unity_cdj", typeof(CdjShow)),
            };
            // Extra materials for the ported scenes: instancing must be on the
            // material asset or the build strips the variant (AGENTS.md).
            var highlandsMat = Mat("Trippin/Highlands", "Highlands");
            var coasterCityMat = Mat("Trippin/CoasterCity", "CoasterCity");
            coasterCityMat.enableInstancing = true;
            var coasterDeckMat = Mat("Trippin/CoasterDeck", "CoasterDeck");
            var coasterNoseMat = Mat("Trippin/CoasterNose", "CoasterNose");
            var showList = new System.Collections.Generic.List<GameObject> { stage, crystals, flowGo, levGo, sculptGo, colGo, tidalGo, stormGo, prismGo, auroraGo, bassGo, pillarsGo };
            var nameList = new System.Collections.Generic.List<string> { "unity_stage", "unity_crystals", "unity_flow", "unity_leviathan", "unity_sculpture", "unity_colossus", "unity_tidal_cathedral", "unity_lightstorm", "unity_prism", "unity_aurora", "unity_basscore", "unity_pillars" };
            foreach (var (kname, ktype) in kitShows)
            {
                var kgo = new GameObject(kname);
                kgo.transform.SetParent(engine.transform, false);
                var ks = (KitShow)kgo.AddComponent(ktype);
                ks.cam = cam;
                ks.beamMat = dir.beamMat;
                ks.glowMat = co.glowMat;
                ks.groundMat = co.groundMat;
                ks.hazeMat = dir.hazeMat;
                ks.structMat = dir.structMat;
                ks.surfaceMat = kitSurface;
                ks.tubeMat = kitTube;
                ks.orbMat = kitOrb;
                ks.membraneMat = kitMembrane;
                ks.ribbonMat = kitRibbon;
                ks.deepMat = kitDeep;
                ks.corridorMat = kitCorridor;
                ks.scopeMat = kitScope;
                ks.landMat = kitLand;
                ks.horizonMat = kitHorizon;
                ks.robotMat = kitRobot;
                ks.screenMat = kitScreen;
                ks.crowdMeshMat = dir.crowdMeshMat;
                if (ks is HighlandsShow hs) hs.highlandsMat = highlandsMat;
                if (ks is FluidShow fl)
                {
                    fl.sim = AssetDatabase.LoadAssetAtPath<ComputeShader>($"{Root}/Shaders/Fluid.compute");
                    if (fl.sim == null) throw new System.Exception("Fluid.compute not found");
                    fl.viewMat = Mat("Trippin/FluidView", "FluidView");
                }
                if (ks is FireworksShow fw)
                {
                    fw.particles = AssetDatabase.LoadAssetAtPath<ComputeShader>($"{Root}/Shaders/HqParticles.compute");
                    if (fw.particles == null) throw new System.Exception("HqParticles.compute not found");
                    fw.sparkMat = Mat("Trippin/HqSpark", "HqSpark");
                    fw.smokeMat = Mat("Trippin/HqSmoke", "HqSmoke");
                    fw.backdropMat = Mat("Trippin/HqBackdrop", "HqSkyline");
                    fw.backdropMat.SetTexture("_MainTex", Tex("HqSkyline.png", TextureWrapMode.Mirror));
                    fw.skyMat = co.skyMat;
                }
                if (ks is InkShow ink) ink.inkMat = Mat("Trippin/HqInk", "HqInk");
                if (ks is JellyfishShow jf)
                {
                    jf.jellyMat = Mat("Trippin/HqJelly", "HqJelly");
                    jf.waterMat = Mat("Trippin/HqDeep", "HqDeep");
                    jf.snowMat = Mat("Trippin/HqSnow", "HqSnow");
                }
                if (ks is CdjShow cj)
                {
                    cj.gearMat = Mat("Trippin/HqGear", "HqGear");
                    cj.clubMat = Mat("Trippin/HqClub", "HqClub");
                    cj.plateTex = GearTex("GearCdjPlate.png");
                    cj.mixTex = GearTex("GearMixPanel.png"); cj.fxTex = GearTex("GearFxScreen.png");
                    cj.screen0 = GearTex("GearCdjScreen0.png"); cj.screen1 = GearTex("GearCdjScreen1.png");
                }
                if (ks is MovingHeadsShow g_movingheads)
                {
                    g_movingheads.gearMat = Mat("Trippin/HqGear", "HqGear");
                }
                if (ks is GrooveboxShow g_groovebox)
                {
                    g_groovebox.gearMat = Mat("Trippin/HqGear", "HqGear");
                    g_groovebox.clubMat = Mat("Trippin/HqClub", "HqClub");
                    g_groovebox.plateTex = GearTex("GearDrumPlate.png");
                }
                if (ks is ControllerShow g_controller)
                {
                    g_controller.gearMat = Mat("Trippin/HqGear", "HqGear");
                    g_controller.clubMat = Mat("Trippin/HqClub", "HqClub");
                    g_controller.plateTex = GearTex("GearCtrlPlate.png");
                    g_controller.uiTex = GearTex("GearLaptopUI.png");
                }
                if (ks is GoldShow g_gold)
                {
                    g_gold.goldMat = Mat("Trippin/HqGold", "HqGold");
                }
                if (ks is LasersShow g_lasers)
                {
                    g_lasers.gearMat = Mat("Trippin/HqGear", "HqGear");
                }
                if (ks is DecksShow dk)
                {
                    dk.gearMat = Mat("Trippin/HqGear", "HqGear");
                    dk.clubMat = Mat("Trippin/HqClub", "HqClub");
                    dk.ttTex = GearTex("GearTTPlate.png"); dk.startTex = GearTex("GearStartBtn.png");
                    dk.mixTex = GearTex("GearMixPanel.png"); dk.fxTex = GearTex("GearFxScreen.png");
                    dk.slipTex = GearTex("GearSlipmat.png");
                    dk.label0 = GearTex("GearLabel0.png"); dk.label1 = GearTex("GearLabel1.png");
                    dk.label2 = GearTex("GearLabel2.png"); dk.label3 = GearTex("GearLabel3.png");
                }
                if (ks is LavaShow lavaShow) lavaShow.lavaMat = Mat("Trippin/HqLava", "HqLava");
                if (ks is MeteorsShow mt)
                {
                    mt.sparkMat = Mat("Trippin/HqSpark", "HqSpark");
                    mt.backMat = Mat("Trippin/HqBackdrop", "HqLake");
                    mt.backMat.SetTexture("_MainTex", Tex("HqLake.png"));
                    mt.particles = AssetDatabase.LoadAssetAtPath<ComputeShader>($"{Root}/Shaders/HqParticles.compute");
                }
                if (ks is HalloweenShow hw)
                {
                    hw.batMat = Mat("Trippin/HqBat", "HqBat");
                    hw.fogMat = Mat("Trippin/HqFog", "HqFog");
                    hw.sparkMat = Mat("Trippin/HqSpark", "HqSpark");
                    hw.backMat = Mat("Trippin/HqBackdrop", "HqHaunted");
                    hw.backMat.SetTexture("_MainTex", Tex("HqHaunted.png"));
                    hw.particles = AssetDatabase.LoadAssetAtPath<ComputeShader>($"{Root}/Shaders/HqParticles.compute");
                }
                if (ks is BubblesShow bu)
                {
                    bu.bubbleMat = Mat("Trippin/HqBubble", "HqBubble");
                    bu.bubbleMat.SetTexture("_Env", Tex("HqStudio.png", TextureWrapMode.Repeat));
                    bu.bubbleMat.SetTexture("_Env2", Tex("HqNeonRoom.png", TextureWrapMode.Repeat));
                    bu.bgMat = Mat("Trippin/HqBubbleBg", "HqBubbleBg");
                    bu.sparkMat = Mat("Trippin/HqSpark", "HqSpark");
                    bu.particles = AssetDatabase.LoadAssetAtPath<ComputeShader>($"{Root}/Shaders/HqParticles.compute");
                }
                if (ks is WinterShow wi)
                {
                    wi.snowMat = Mat("Trippin/HqSnow", "HqSnow");
                    wi.backMat = Mat("Trippin/HqBackdrop", "HqVillage");
                    wi.backMat.SetTexture("_MainTex", Tex("HqVillage.png"));
                }
                if (ks is FirefliesShow ff)
                {
                    ff.flyMat = Mat("Trippin/HqFirefly", "HqFirefly");
                    ff.backMat = Mat("Trippin/HqBackdrop", "HqForest");
                    ff.backMat.SetTexture("_MainTex", Tex("HqForest.png"));
                }
                if (ks is FireShow fire)
                {
                    fire.fireMat = Mat("Trippin/HqFire", "HqFire");
                    fire.sparkMat = Mat("Trippin/HqSpark", "HqSpark");
                    fire.smokeMat = Mat("Trippin/HqSmoke", "HqSmoke");
                    fire.particles = AssetDatabase.LoadAssetAtPath<ComputeShader>($"{Root}/Shaders/HqParticles.compute");
                }
                if (ks is SynthwaveShow sw)
                {
                    sw.synthMat = Mat("Trippin/HqSynth", "HqSynth");
                    sw.skyMat = Mat("Trippin/HqBackdrop", "HqSynthSky");
                    sw.skyMat.SetTexture("_MainTex", Tex("HqSynthSky.png"));
                }
                if (ks is PoolShow poolShow)
                {
                    poolShow.poolMat = Mat("Trippin/HqPool", "HqPool");
                    poolShow.poolMat.SetTexture("_Tiles", Tex("HqPoolTiles.png", TextureWrapMode.Repeat));
                }
                if (ks is RainShow rn)
                {
                    rn.rainMat = Mat("Trippin/HqRain", "HqRain");
                    rn.rainMat.SetTexture("_Street", Tex("HqRainStreet.png"));
                }
                if (ks is ConfettiShow cf)
                {
                    cf.particles = AssetDatabase.LoadAssetAtPath<ComputeShader>($"{Root}/Shaders/HqParticles.compute");
                    cf.confettiMat = Mat("Trippin/HqConfetti", "HqConfetti");
                }
                if (ks is ChromeShow chs)
                {
                    chs.chromeMat = Mat("Trippin/HqChrome", "HqChrome");
                    chs.chromeMat.SetTexture("_Env", Tex("HqStudio.png", TextureWrapMode.Repeat));
                    chs.chromeMat.SetTexture("_Env2", Tex("HqNeonRoom.png", TextureWrapMode.Repeat));
                }
                if (ks is KaleidoShow kal)
                {
                    kal.kaleidoMat = Mat("Trippin/HqKaleido", "HqKaleido");
                    kal.shardMat = Mat("Trippin/HqShard", "HqShard");
                    kal.glassMat = Mat("Trippin/HqShardGlass", "HqShardGlass");
                    kal.backMat = Mat("Trippin/HqBackdrop", "HqLightBox");
                    kal.glassA = Tex("HqGlassDichroic.png", TextureWrapMode.Mirror);
                    kal.glassB = Tex("HqGlassStained.png", TextureWrapMode.Mirror);
                }
                if (ks is MarbleShow mb)
                {
                    mb.sim = AssetDatabase.LoadAssetAtPath<ComputeShader>($"{Root}/Shaders/Fluid.compute");
                    mb.marble = AssetDatabase.LoadAssetAtPath<ComputeShader>($"{Root}/Shaders/Marble.compute");
                    if (mb.sim == null || mb.marble == null) throw new System.Exception("Fluid.compute / Marble.compute not found");
                    mb.viewMat = Mat("Trippin/MarbleView", "MarbleView");
                }
                if (ks is CoasterShow cosh)
                {
                    cosh.coasterCityMat = coasterCityMat;
                    cosh.deckMat = coasterDeckMat;
                    cosh.noseMat = coasterNoseMat;
                    cosh.skyMat = Mat("Trippin/Sky", "Sky");
                }
                showList.Add(kgo);
                nameList.Add(kname);
            }
            mgr.shows = showList.ToArray();
            mgr.names = nameList.ToArray();

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
                // The report's errors aren't always in the log (shader errors on
                // another graphics API, for one): print them.
                foreach (var step in report.steps)
                    foreach (var m in step.messages)
                        if (m.type == LogType.Error || m.type == LogType.Exception)
                            Debug.Log("[StageBuilder] error: " + m.content.Split('\n')[0]);
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
