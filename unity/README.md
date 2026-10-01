# Trippin Stage (Unity prototype)

A live festival main stage rendered in Unity, driven by Trippin and shown in Trippin as the `unity_stage` scene.

- **Trippin → Unity:** `src/link.rs` sends one JSON datagram per frame to 127.0.0.1:9137, with the audio vocabulary, beat/bar, palette and cut flag. `TrippinLink.cs` receives it and pushes shader globals (`_TBeat`, `_TLvl`, `_TPal`…). With no feed it runs a synthetic 126 BPM show.
- **Unity → Trippin:** KlakSpout sends the 1920x1080 output as Spout "Trippin Stage". `spout::Receiver` puts it into `ext_tex`, and `unity_stage.wgsl` undoes present's ACES so it passes through unchanged. Turn it on with Settings `unity_link` (Windows only; macOS would need NDI/Syphon).
- **The stage** (`StageDirector.cs`) is built in code at startup:
  - static architecture;
  - LED walls (`LedWall.shader`) with a new pattern every 8 bars;
  - 20 lasers with a new formation every 4 bars, morphing in over a beat (`Beam.shader`);
  - light shafts, haze and an instanced crowd (`Crowd.shader`);
  - pyro, CO2 and fireworks on drops;
  - camera shots that change every 8 bars.

Build from the CLI (Unity 6000.3.25f1):

    unity run . -- -executeMethod TrippinStage.EditorTools.StageBuilder.BuildPlayer [-stageOut Build2]

Run `Build/TrippinStage.exe`. `-record <dir> -recordSeconds 64` writes a fixed-30-fps JPEG sequence and quits (encode with ffmpeg).
