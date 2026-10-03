# Trippin Stage (Unity prototype)

A Unity engine of festival-grade shows, driven live by Trippin and shown in Trippin as `unity_*` scenes. Each show is a child of the Engine object named after its Trippin scene. `ShowManager` switches to whichever one Trippin's feed names; with no feed it cycles them every 16 bars. `-show <name>` pins one.

| Show / Trippin scene | What |
|---|---|
| `unity_stage` | The festival main stage (below). |
| `unity_crystals` | Screen content: a flight through a spiralling tunnel of faceted chrome crystals with a light at the end (`CrystalShow.cs`, `Chrome.shader`). Bass swells the crystals, the spiral swings with phrases, drops bloom it outward. |
| `unity_flow` | Screen content: 262k GPU particles forming shapes (sphere, torus, helix, galaxy, gyroscope) that morph every 4 bars (`FlowShow.cs`, `Flow.compute`, `Points.shader`). Turbulence wobbles target slots (not a force field — fields with sinks clump particles into threads); drops scatter and re-form. |


- **Trippin → Unity:** `src/link.rs` sends one JSON datagram per frame to 127.0.0.1:9137, with the audio vocabulary, beat/bar, palette and cut flag. `TrippinLink.cs` receives it and pushes shader globals (`_TBeat`, `_TLvl`, `_TPal`…). With no feed it runs a synthetic 126 BPM show.
- **Unity → Trippin:** KlakSpout sends the 1920x1080 output as Spout "Trippin Stage". `spout::Receiver` puts it into `ext_tex`, and `unity_stage.wgsl` undoes present's ACES so it passes through unchanged. Turn it on with Settings `unity_link` (Windows only; macOS would need NDI/Syphon).
- **The stage** (`StageDirector.cs`) is built in code at startup:
  - static architecture;
  - LED walls (`LedWall.shader`) with a new pattern every 8 bars;
  - 20 lasers with a new formation every 4 bars, morphing in over a beat (`Beam.shader`);
  - light shafts, haze and an instanced crowd (`Crowd.shader`);
  - pyro, CO2 and fireworks on drops;
  - camera shots that change every 8 bars;
  - set pieces: a sun disc and ring behind the set (`Sun.shader`) that blooms on drops; god-ray sweeps strongest in breakdowns; a kinetic 6x8 LED tile rig rippling over the deck; instanced phone lights over the crowd in breakdowns (`Phones.shader`); confetti cannons plus an overhead confetti release on drops (`Confetti.shader`).

## Following the music

Tick **Settings → Director → Unity engine link** in Trippin. That's all — Trippin (`src/engine.rs`) launches the player headless (`-batchmode`; `ShowManager` renders the camera explicitly since batch mode doesn't), restarts it if it dies, and kills it when the link goes off or Trippin quits; the player also quits if Trippin's feed stops for 15 s.

- **Feed:** show state (bands, kicks, beat/bar, palette, current scene) over UDP to the port passed as `-trippinPort`.
- **Frames:** back through a memory-mapped file passed as `-trippinFrame` (`FrameExporter.cs`: async GPU readback, rows flipped to top-first, written under a seqlock). Works the same on Windows and macOS — no Spout/Syphon.
- **Scenes:** the `unity_*` scenes join the auto-pilot rotation while frames arrive; when Trippin cuts to `unity_flow`, Unity switches to that show.

Users never install it: if no player is found, Trippin downloads this platform's zip from the `unity-engine-vN` GitHub release (checksum pinned in `engine.rs::ASSET`), unpacks it into `<data dir>/unity/` and launches it — about 36 MB on Windows, roughly 10 s. `TRIPPIN_ENGINE_FRESH=1` ignores local builds to test that path.

**Publishing a new engine version** (when the Unity project changes):
1. Build Windows (and `-stageMac`).
2. Zip each build's contents, without `*_DoNotShip`. On the Mac zip, mark `Contents/MacOS/*` executable.
3. `gh release create unity-engine-vN … --latest=false`.
4. Bump `RELEASE_BASE` and the `ASSET` names and checksums. Existing installs update themselves: the download writes `ENGINE_VERSION` (the asset name) into `<data dir>/unity/`, and a folder whose stamp doesn't match `ASSET` is downloaded again.

Trippin looks for the player next to itself (`unity/TrippinStage.exe`, or `Contents/Resources/unity/TrippinStage.app` on macOS), then in `<data dir>/unity/`, then this folder's `Build/` (`BuildMac/` on macOS).

Music comes from Trippin's normal audio capture (system output, or the Audio in device on the Settings tab). With no feed, Unity plays a synthetic 126 BPM demo cycling its shows.

Build from the CLI (Unity 6000.3.25f1):

    unity run . -- -executeMethod TrippinStage.EditorTools.StageBuilder.BuildPlayer [-stageOut Build2] [-stageMac]

Run `Build/TrippinStage.exe`. `-record <dir> -recordSeconds 64` writes a fixed-30-fps JPEG sequence and quits (encode with ffmpeg).

## Crowd meshes

The stage crowd uses generated people from Alice (the owner's generation server; external agent API `POST /api/agent/crowd {"count": N}` at alice.deviousweb.com, key in the untracked `alice.env`; Cloudflare rejects Python's default User-Agent, so send a curl-like one). Prepare a batch zip with:

    python tools/crowd_meshes.py crowd.zip --prefix a2

It rejects failed reconstructions, bakes textures into vertex colours, decimates to about 2.5k triangles and writes `Assets/Trippin/Resources/Crowd/*.bytes`. StageDirector picks them up on the next build. Hunyuan3D's people face +z (the stage). `-stageShot 0` pins the wide shot to check them in a recording.

## Colossus (giant android)

`unity_colossus` is an original armoured robot made with Alice. A Flux.2 reference image (full body, fists raised, plain white background) went through `POST /agent/mesh` with that image to make a textured Hunyuan3D GLB. The API is documented in `aliceapi.md` and the key lives in the untracked `alice.env`.

To prepare it:

    python tools/android_mesh.py android.glb --smooth 6

This bakes the texture into vertex colours. It also estimates an 11-bone skeleton and skin weights, and cuts the head away, because Unity puts the sculpted face (`AndroidHead.shader`) on the neck bone. The skeleton heuristics expect a standing pose with the arms raised.

To check the weights before building, pose the mesh offline: lower the arms and bend the knees, then render it. ColossusShow poses the skeleton every frame from the music.
