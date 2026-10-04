# Trippin Stage (Unity prototype)

A Unity engine of festival-grade shows, driven live by Trippin and shown in Trippin as `unity_*` scenes. Each show is a child of the Engine object named after its Trippin scene. `ShowManager` switches to whichever one Trippin's feed names; with no feed it cycles them every 16 bars. `-show <name>` pins one.

| Show / Trippin scene | What |
|---|---|
| `unity_stage` | The festival main stage (below). |
| `unity_crystals` | Screen content: a flight through a spiralling tunnel of faceted chrome crystals with a light at the end (`CrystalShow.cs`, `Chrome.shader`). Bass swells the crystals, the spiral swings with phrases, drops bloom it outward. Behind it all a `Nebula` dome of wisps and stars, and `Points` dust motes stream past the lens. |
| `unity_flow` | Screen content: 262k GPU particles forming shapes (sphere, torus, helix, galaxy, gyroscope) that morph every 4 bars (`FlowShow.cs`, `Flow.compute`, `Points.shader`). Turbulence wobbles target slots (not a force field — fields with sinks clump particles into threads); drops scatter and re-form. A `Nebula` dome gives it depth and a `Backglow` core light burns at the shape's heart. |
| `unity_leviathan` | Screen content: a manta-like creature of light (`LeviathanShow.cs`, `Leviathan.compute`, `LevSkin`/`Filament` shaders) swimming a dark void of drifting motes. Kicks send a glow pulse head-to-tail; drops flare the strands. |
| `unity_sculpture` | Screen content: a morphing monolith on a mirror flat at night (`SculptureShow.cs`, `Sculpture.shader`) — night sky, wet floor carrying its reflection, a searchlight ring raking the sky, ground mist and a horizon glow that blooms on drops. |
| `unity_colossus` | Screen content: an armoured android (Alice mesh, 11-bone skinning) in a night city (`ColossusShow.cs`, `Android`/`AndroidHead`/`City`/`Sky`/`Ground` shaders) — lit towers, rooftop searchlights, a wet plaza floor. |
| `unity_tidal_cathedral` | Screen content: four pairs of fixed pointed glass-vault ribs framing asymmetric moving membranes over a dark sea with broken light reflections and an Alice-generated stained-glass rose window at the far end (`TidalCathedralShow.cs`, `Textures/TidalRose.png`, `TidalFrame`/`TidalSail`/`TidalSea`/`TidalSky`/`TidalCore` shaders). Bass widens the membranes, mids send folds through them, highs sharpen the edge sheen, and the camera travels through the aisle on the flow clock. Eight fixed laser fixtures sweep through smoky 4-bar formations with smoothed energy, never beat flashes. |
| `unity_lightstorm` | A light show where the rig is the subject: 48 moving heads (three overhead trusses plus a floor ring), each a wide haze cone plus a thin laser core, splashing on a wet floor (`LightstormShow.cs`, reusing `Beam`/`Backglow`/`Ground`/`Haze`). A new geometric formation every 4 bars (curtain, helix, fan, cathedral, crossing sheets, X-weave). Cone width follows slow bass presence; aim rides `clock4`; the camera orbit swings with phrases. |
| `unity_prism` | A laser cage: 96 thin segments strung between ceiling and floor emitters so the beams draw geometry, reflected in a wet floor (`PrismShow.cs`, reusing `Beam`/`Ground`/`Haze`). A new formation every 4 bars morphing in over a beat (twisting hyperboloid, tent, bending curtain, double cone, rippling forest, braid). The twist and ripple follow slow bass presence; the cage turns on `clock4` with a phrase swing that reverses; builds pull it into a tight spindle and the drop blows it open. |
| `unity_aurora` | The breakdown show: five slow aurora curtains folding over a still reflective floor with six searchlights sweeping the sky (`AuroraShow.cs`, `Aurora.shader`: a grid ribbon folded in the vertex shader, rays and palette in the fragment). Fold depth follows slow bass, height follows mids, travel rides `clock4` and reverses on phrases; brighter in breakdowns, subdued when the drums play; builds gather the curtains and the drop flares them and fans the searchlights. |
| `unity_basscore` | The bass-drop show: a big circular wire membrane (a sub-bass plate) over a reflective floor with 24 light shafts rising round it (`BasscoreShow.cs`, `Membrane.shader`: a polar grid displaced in the vertex shader, drawn as bright rings and spokes). Ripple depth follows slow bass; each kick launches a ring pulse that travels out across the plate; the ripple travels on `clock4` and reverses on phrases; builds gather and dim it, the drop sends three big staggered rings and flares the shafts. |
| `unity_pillars` | A 15x15 field of light pillars (Beam shader only) whose heights ripple as a landscape: a radial wave plus a cross wave, depth on slow bass, both travelling on `clock4` and reversing on phrases (`PillarsShow.cs`). A build sinks the field to a dim carpet; the drop sends a tall ring front racing outward from the centre. |
| `unity_orbit_foundry` | Screen content: a molten metal core suspended inside three precessing gimbal rings, welded by beams fired from four pods on a fixed outer gantry, over an etched foundry pad (`OrbitFoundryShow.cs`, `OrbitCore`/`OrbitRing`/`OrbitFloor` shaders). The core is a fixed sphere shaded as mirror-ball facets (flat `ddx`/`ddy` normals) with ember seams; it slowly tumbles while bass and kick breathe its radius, mids drive ring precession, highs steer the weld points and shed sparks, kicks compress the core and pull the beams to a common strike zone. The gantry and floor never move. |
| `unity_helix` | A double helix of light (DNA): glowing tube strands, base-pair rungs, nodes and rising motes, reflected in a wet floor (`HelixShow.cs`, `Tube.shader`). Radius swells with bass and kick and bulges where the spectrum is loud; spins on the energy clock, reversing with phrases; every segment glows with its own band. |
| `unity_tesseract` | A rotating 4D hypercube (and a smaller one inside it) projected to 3D in thin lasers (`TesseractShow.cs`). Projection distance follows the bass; six rotation planes turn on the energy clock and swing with phrases. |
| `unity_polyhedra` | An icosahedron, an octahedron and a cube nested and tumbling (`PolyhedraShow.cs`). Every vertex is pushed out by a spectrum band; edges glow with the corners they join. |
| `unity_knot` | A thick torus knot of light with a comet head racing round it, a thin counter-rotating ghost knot and streaming beads; a new (p, q) winding every 8 bars morphing over two beats (`KnotShow.cs`, `Tube.shader`). The knot is a live spectrum along its length; tube swells with bass and kick, ripples with highs. |
| `unity_gyroscope` | Five glowing tube rings turning about different axes round a plasma core, with orbiting beads (`GyroscopeShow.cs`, `Tube.shader`, `Orb.shader`). Each ring breathes with one band and is a spectrum wrapped round its circumference, turns on a band energy clock, tilt swings with phrases. |
| `unity_lattice` | A 5x5x5 lattice of glowing nodes joined by thin lasers (`LatticeShow.cs`). Waves travel through it on the energy clock, depth follows bass, and shells push out with the spectrum from the core (lows) to the corners (highs). |
| `unity_pendulum` | A harmonograph: three pens each draw a long tapering ribbon of light, weaving Lissajous knots (`PendulumShow.cs`, `Tube.shader`). Swing amplitude per axis follows a band; time is the energy clock; the frequency ratios change every 8 bars. |
| `unity_spectrum` | Three rings of vertical light bars, the live spectrum wrapped round a circle, with the outer rings showing it a fraction of a second ago (`SpectrumShow.cs`). |
| `unity_eclipse` | A solar eclipse: a black disc, a soft halo and 96 corona streamers whose lengths follow the spectrum (`EclipseShow.cs`). The disc uses the stage's dark Structure material; verify it hides the halo. |
| `unity_lightrain` | 240 streaks of light falling through haze with a wet-floor reflection (`LightRainShow.cs`). Fall is a function of the energy clock; length follows loudness and bass; the slant swings with phrases. |
| `unity_warp` | A hyperspace flight of 320 light streaks (`WarpShow.cs`). A flight scene, so no beat flashes: speed is the energy clock, length follows loudness; camera rolls with phrases. |
| `unity_ledwall` | A 36x20 LED wall showing spectrum bars, ripples, plasma and diamond waves, a new one every 4 bars (`LedWallShow.cs`). |
| `unity_fountain` | Three fountains throwing 360 glowing drops in ballistic arcs (`FountainShow.cs`). Launch speed follows bass and mids when each drop leaves the nozzle; nozzles lean with phrases. |
| `unity_galaxy` | A 700-star spiral galaxy with a glowing core, turning differentially (`GalaxyShow.cs`). Spiral winding follows the slow bass; stars swell with highs. |
| `unity_nebula` | A deep-space nebula as a full-screen pass: three domain-warped smoke layers tied to slices of the spectrum, a core with a drop ring, a galactic band and stars (`NebulaShow.cs`, `DeepSpace.shader`). |
| `unity_terrain` | A low flight over a shaded neon mountain range toward a huge banded sun (`TerrainShow.cs`, `Land.shader` mode 0, `Horizon.shader`). Flight scene: no beat flashes. The ranges left and right are a spectrum; flight speed is the energy clock; the range heaves with the bass and on drops. |
| `unity_ocean` | A night sea under a huge moon with four sweeping searchlights (`OceanShow.cs`, `Land.shader` mode 1, `Horizon.shader`). Eight swells each driven by a spectrum slice (bass = long swells, highs = chop); kicks launch ripples; wind direction turns with phrases. |
| `unity_chladni` | A vibrating plate whose glowing lines are the nodal lines of a Chladni figure, a new mode every 4 bars (`ChladniShow.cs`, `WireSurface.shader` mode 2). |
| `unity_tunnel` | A flight down a snaking sci-fi corridor of lit panels toward a light, a full-screen pass (`TunnelShow.cs`, `Corridor.shader`). Flight scene: no beat flashes. Ribs deepen with bass, the wall panels are a spectrum wrapped round the circumference; speed is the energy clock. |
| `unity_orb` | A plasma orb over a reflective floor with two precessing light rings (`OrbShow.cs`, `Orb.shader`). Surface displacement follows bass, grain follows highs. |
| `unity_radar` | A radar scope: a sweep with phosphor persistence, a polar spectrum plot the sweep lights up, contacts, kick rings and a 3D wall of light bars round the rim (`RadarShow.cs`, `Scope.shader`). |

Future standalone show ideas (not yet built):
- **Glass Ocean:** immense translucent swimming organisms whose bodies contract with bass and whose fins ripple with highs; no camera-rushing objects.
- **Fabric Storm:** one iridescent sheet develops broad bass folds and fine high-frequency creases; drops pull it toward new configurations.
- **Magnetic Desert:** a hovering abstract core morphs and draws dark sand into flowing field lines; the landscape remains fixed.

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

## Kit shows (`Kit.cs`)

`KitShow` is the base for the shows added in bulk: it ticks one `Rx` per show (the music as that show sees it) and owns the camera rig, floor and haze. `Rx` carries the eased fast signals every show must react to on real music (`bassFast`/`midFast`/`mhFast`/`highFast`, `kick`, the eased spectrum `Spec(x)`, overall loudness `lum`) alongside the slow ones (`*Slow`, `clk` energy clock, `phrase`) and the build/drop state (`tension`, `impact` already given its 0.25 s attack). `Rx.Gain()` is the shared luminance: it follows loudness, dips through a build and flares on the drop. `BeamPool`, `GlowPool` and `HazeSet` are fixed-count pools of the existing Beam/Backglow/Haze materials. New shaders: `WireSurface` (terrain, ocean, Chladni plate), `WireTube`, `Orb`. They're all registered by one loop in `StageBuilder`.

## Build and drop (`DropDirector.cs`)

`ShowManager` ticks a shared `DropDirector` every frame, so any show can read `Tension` (0..1 slow integrator: climbs through breakdowns and rising `build`, drains otherwise), `Impact` (1 at the drop, gone after ~6 beats) and `Dropped` (one frame). `build` from the audio is only a fast-vs-slow energy trend, not a riser detector, so the drop itself is still "drums return after >4 s out"; Tension provides the anticipation before it. `unity_stage` and `unity_lightstorm` pull their beams to one point, thicken the fog and creep the camera in on Tension, then flare on Impact. Flight/tunnel shows may use these for shape and motion only, never strobes.

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

Run `Build/TrippinStage.exe`. With no Trippin feed it cycles every 16 bars; `-cycleBars N` changes that and `-tourShows a,b,c` cycles only the named shows, for a quick recorded tour (e.g. `-tourShows unity_helix,unity_warp -cycleBars 4 -record tour -recordSeconds 30`). `-uncapped` removes the 60 fps cap so the `[Stage] ms/frame` log shows the real cost. `-record <dir> -recordSeconds 64` writes a fixed-30-fps JPEG sequence and quits (encode with ffmpeg).

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
