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

Build from the CLI (Unity 6000.3.25f1):

    unity run . -- -executeMethod TrippinStage.EditorTools.StageBuilder.BuildPlayer [-stageOut Build2]

Run `Build/TrippinStage.exe`. `-record <dir> -recordSeconds 64` writes a fixed-30-fps JPEG sequence and quits (encode with ffmpeg).
