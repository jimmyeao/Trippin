<p align="center"><img src="logo.png" width="200" alt="Trippin logo"></p>

# Trippin

Live, music-reactive visuals for DJ sets. Trippin listens
to whatever your DJ software is playing and drives GPU shader scenes from it. It
tracks band energy, onsets, tempo and beat phase, and an auto-pilot cuts between
scenes on phrase boundaries and drops.

## Install

Download `Trippin-Setup-<version>.exe` from the GitHub **Releases** page (or
from the artifacts of the latest *Build installer* workflow run) and run it.
Settings are saved to `%APPDATA%\Trippin	rippin.json`.

To release a new version: **Actions → Build installer → Run workflow**, enter
the version (e.g. `0.2.0`) — it bumps `Cargo.toml`, commits, tags `v<version>`,
builds the installer and publishes the release. Or do it by hand: bump
`version` in `Cargo.toml`, then `git tag v0.2.0 && git push origin v0.2.0`.
Every push to `master` also builds an installer artifact.

Build the installer locally (needs Inno Setup 6):

```
cargo build --release --features gui
"C:\Program Files (x86)\Inno Setup 6\ISCC.exe" /DAppVersion=0.1.0 installer	rippin.iss
```

### macOS

CI also builds `Trippin.app` as a universal binary (Intel + Apple Silicon) —
grab `Trippin-macOS-<version>.zip` from releases or workflow artifacts, unzip,
and drag to Applications. CI artifacts are signed and notarized when the
signing secrets below are configured, so they open like any other app;
unsigned builds still hit Gatekeeper — `xattr -dr com.apple.quarantine
Trippin.app`, or attempt to open then **System Settings → Privacy &
Security → Open Anyway**.

#### Signing secrets (repo → Settings → Secrets and variables → Actions)

- `APPLE_CERTIFICATE` — base64 of a **Developer ID Application** cert +
  private key exported as `.p12` (Keychain Access → export, or Xcode →
  Manage Certificates → Developer ID Application first). Encode with
  `base64 -i cert.p12 | pbcopy`.
- `APPLE_CERTIFICATE_PASSWORD` — the `.p12` export password.
- `APPLE_SIGNING_IDENTITY` — e.g. `Developer ID Application: Name (TEAMID)`;
  `security find-identity -v -p codesigning` prints the exact string.
- `APPLE_ID` — the Apple ID email for notarization.
- `APPLE_PASSWORD` — an **app-specific password** for that account
  (appleid.apple.com → Sign-In and Security → App-Specific Passwords).
- `APPLE_TEAM_ID` — the 10-char team ID (developer.apple.com → Membership).

Settings live in `~/Library/Application Support/Trippin/trippin.json`. Shaders
and dancers resolve from the bundle's `Contents/Resources/`; running from a
checkout uses the repo directories as before.

**Audio:** macOS captures the system output mix directly via ScreenCaptureKit
— the same API OBS uses — so no mic or BlackHole loopback is needed. macOS
prompts once for screen/system-audio recording permission on first launch
(macOS 13+; on older systems it falls back to the default input). `--mic`
forces the microphone, and `--device "<name>"` picks a specific input or
interface for a DJ booth-out. `--list-devices` shows what's available.
The panel key is **P** on macOS (F1 is a brightness key on Touch Bar
machines; Fn+F1 also works). On macOS, presents are ungated from vsync and
the render
loop self-paces at the display's refresh — vsync-gated presents stall ~2
frames whenever the compositor is loaded (e.g. another app fullscreen on a
second display); `--vsync` restores them if that ever causes trouble.

## Run

```
cargo run --release                         # capture what you hear (loopback / system audio)
cargo run --release -- --list-devices       # list capture devices
cargo run --release -- --device "Serato"    # a specific input (or output-as-loopback)
cargo run --release -- --mic                # force the default input instead of system audio
cargo run --release -- --scene tunnel       # start on a scene, auto-pilot off
cargo run --release -- --dancer neon --canon  # force a dancer look (auto-pilot changes it on cuts)
cargo run --release -- --no-dancer          # start with the dancer layer off
cargo run --release -- --no-panel           # no control panel window
cargo run --release -- --gpu low            # use the integrated GPU (renders at 75% by default)
cargo run --release -- --scale 0.6          # scene render resolution, upscaled to the window
cargo run --release -- --vsync              # macOS: vsync-gated presents instead of the default
cargo run --release -- --fullscreen
cargo run --release -- --song track.wav   # open a track on the Timeline tab
cargo run --release -- --analyze track.mp3   # print the per-bar feature summary sent to the AI
cargo run --release -- --ai-build track.mp3  # run the AI show build end-to-end, print the cues
cargo run --release -- --snap storm_front,deep_blue --snap-size 1920x1080  # headless PNG + ms/frame
```

`--snap <scenes|all>` renders scenes offscreen through the real scene → bloom
→ present chain with a synthetic 126 BPM groove, writes PNGs to `snaps/`
(`--snap-out`), and times each scene (`--snap-bench N` frames, results also in
`snaps/bench.tsv`). `--snap-at 2,6` picks the capture times in seconds and
`--palette` picks the palette. There's no window and no audio device, so it
works for GPU budgeting on any machine.

Drag the visuals window to the projector / LED wall and press **F**. A
**control panel** window opens alongside it (F1 shows/hides it), organised
into tabs — **Show** (modes, scene stepping, length, blackout, fullscreen,
latency/downbeat), **Scenes** (the playlist: tick to include, search filter,
"show" to jump to one now), **Dancer** (on/off, look, canon, size, which
routines), **Effects** (the post effect + strength — picks apply live to the
output, so the panel doubles as a preview) and **Keys** (rebindable hotkeys:
click Rebind, then press a key). Everything is saved to `trippin.json`, and
the visuals keep animating while the panel is being moved — rendering runs
on its own thread.

**Modes:** *Auto* cuts scenes every phrase and early on drops, and the dancer
follows the track. *Static* holds the current scene while the dancer still
changes with the phrases. *Manual* changes nothing by itself.

Default keys (all rebindable in the panel):

| Key | Action |
|---|---|
| → / ← | next / previous scene (next is random in random order) |
| A / H / M | mode: auto / static (hold) / manual |
| R | random ↔ sequential scene order |
| D | dancer on / off |
| C / S / V | next routine / next look / canon auto→on→off |
| B | blackout (fade to black and back) |
| F | fullscreen on / off |
| Space | mark this beat as the downbeat |
| [ / ] | latency −/+ 5 ms |
| X | cycle the post effect (off → mirrors → kaleido) |
| F5 | reload shaders |
| T | timeline play / pause |
| G | timeline record on / off |
| F1 (P on macOS) | show / hide the control panel |
| Esc | leave fullscreen (it never quits; close the window to quit) |

## Scenes

- **Tunnels / flights:** `neon_portals` (neon triangle, square and hexagon
  portals over a wet reflective floor; the shape and twist change per cut),
  `block_tunnel` (a curving tunnel of jutting boxes, either red monochrome or
  dark metal with neon edges), `fractal_flight`, `portal_zoom` (endless Droste
  portal zoom), `ring_runner` (a warp tunnel of glowing gate rings that pulse
  per band), `tunnel` and `synthwave`. These move on `u.flow`, a smooth
  tempo clock that never jumps, and they have no beat flashes, so the flight
  stays fluid.
- **Scenery:** `ocean` (sunset sea), `clouds` (sunset cloud flight),
  `city_rain` (neon street in the rain, wet-asphalt reflections),
  `highlands` (misty mountain valley at dawn), `dunes` (desert at dusk),
  `beach` (palm silhouettes and surf), `aurora` (northern lights whose
  curtains fold and shimmer to the spectrum over a mountain lake) and
  `laser_show` (festival beam fans over a crowd). Flight
  scenes ride `u.flow`; nothing flashes.
- **Mandalas / abstract:** `fire_mandala` (a fire-and-ice kaleidoscope),
  `julia_portal`, `kaleido`, `fluid` and `spectrum_rings`. These still pulse
  with the beat.
- **Analysers** — scenes that draw the music itself: `eq_bars` (mirrored
  spectrum bars), `eq_round` (radial analyser), `eq_skyline` (a night skyline
  of glowing spectrum towers), `scope` (phosphor oscilloscope), `spectrogram`
  (scrolling waterfall), `lissajous`, `vu` (giant bass/mid/high meters),
  `led_wall`, `waveshaper`, `polar_bloom` and `levels`.
- **Pulse / particle:** `shockwaves`, `pulse_grid`, `rings`,
  `bounce` (orbs hopping on a lit floor), `ripple` (beat-spawned water
  rings), `stardrive`, `orbiters`, `ribbons`, `ink`, `flare_ring`
  (chromatic ring bursts on the beat), `beam_sweep` (festival searchlights),
  `metaballs`, `sparks` (pyro fountains), `sun_rays` (a blazing sun with a
  spectrum corona), `helix` (a spiralling strand that breathes wide on the
  beat), `comets` (arc-tailed comets on the beat),
  `voronoi_pulse`, `chevrons`, `pixel_fall` (slow glyph rain),
  `glitch_grid` (a tearing LED
  tile wall), `light_trails` (long-exposure light streaks) and
  `bokeh_lights` (soft out-of-focus orbs — a mellow breakdown look).
  Ring bursts ride the beat, onset splats and fountains fire on drops,
  trails live in the feedback buffer.
- **Raymarched 3D** (marked `// @heavy`, automatically gated to GPUs that can
  afford them — override on the Scenes tab): `rave_hall` (a corridor of
  banded pillars), `gyroid_drift`, `bass_blocks`, `chrome_bloom` (a metal
  flower that opens on the kick), `prism_field` (a 3D equaliser landscape),
  `gyro_core` (nested neon rings around a molten core), `wire_terrain`
  (a neon wireframe terrain flyover), `crystal_cave` (a jewelled cavern
  flythrough), `canyon_run` (sprinting through a winding lit canyon),
  `bubble_room` (a room of floating orbs bobbing on the beat), `lattice`
  (an infinite
  glowing cube lattice), `arch_run` (a cathedral vault of arches) and
  `torus_dance` (a band-lit torus knot spinning centre-screen).
- **2026 tier** (photoreal, all `@heavy`; they use the bloom pass, AgX tone
  mapping and the baked noise volumes). `salt_flats`: a mirror-still salt
  flat under the Milky Way with a spectrum-display monolith. `orbit_night`:
  low orbit before dawn, with city lights, storms flashing on the kicks and a
  scattering atmosphere limb. `chrome_ferro`: a liquid-chrome ferrofluid blob
  whose spike rings follow the spectrum, in a photo studio. `warehouse_haze`:
  moving-head beams in haze inside a concrete warehouse, using analytic
  volumetric beams. `stage_rig`: a festival main stage from the crowd, with
  an LED wall, a beam rig, flame jets on the drops and backlit hands.
  `event_horizon`: a lensed black hole accretion disk. `glass_monoliths`:
  refracting glass slabs with RGB dispersion in front of a spectrum light
  wall. `storm_front`: a volumetric supercell over the sea, with lightning
  inside the cloud on the big hits. `neon_alley`: a rain-soaked brick alley
  with blade neon signs and puddle reflections. `deep_blue`: underwater,
  with caustics, god rays, marine snow and pulsing jellyfish. `glacier_cave`:
  a scalloped ice tunnel lit through the ice. `megastructure`: a dusk canyon
  through an endless brutalist structure with spectrum windows. Each is
  ≤1 ms/frame at 1080p on an RTX 5070 Ti. Scaling by FP32 throughput, that
  estimates ~3 ms on an RTX 3060 and ~12 ms on an M2, so they should hold
  60 fps on the floor hardware at full res.
- **Graphic / LED-wall:** `dot_field` (a spectrum-driven LED wall),
  `grid_flash` (an LED dancefloor), `warp_grid` (a tron horizon rush),
  `strobe_bars` (a wall of light towers that grow with their band),
  `stripes_flow`,
  `moire` (interference rings), `sunburst` (a rotating ray fan), `pinwheel`,
  `hypno` (a spinning hypnosis disc), `arc_sweep` (radar rings), `barcode`
  (a living spectrum barcode), `led_chase` (chase lights around the frame
  edge), `vortex` (a polar whirlpool) and `plasma`.
- **Organic / atmospheric:** `nebula` (billowing deep-space gas),
  `caustics` (pool-light shimmer), `sunset_waves` (a retro sea under a
  striped sun, sea on the bottom third), `lightning`
  (forked strikes that re-fire on onsets), `confetti` (tumbling bursts),
  `ember_rise` (embers climbing off a fire pit), `fire_wall` (a rising wall
  of flame), `aurora_wave` (northern-light curtains overhead),
  `spiral_galaxy`, `tide_lines` (rolling light swells)
  and `data_fall` (cyber-rain columns).
- **Pseudo-3D** (projected, no marching — cheap on iGPUs): `cube_spin`
  (tumbling wireframe cubes) and `dot_wave` (a stadium-crowd wave of dots).
- **Seasonal:** `halloween`, `christmas` and `fireworks` only enter the
  playlist in season (October, December, Bonfire Night and New Year; see
  `Seasonal` in the panel). They're the most audio-reactive scenes:
  beat-flickering lanterns, onset lightning, and bursts that barrage on drops.

Heaviest on integrated graphics (at the automatic 75% scale): `block_tunnel`
~57 fps and `fire_mandala` ~84 fps at 2560×1440. Every scene holds 144 fps on
an RTX 5070 Ti.

When writing a scene, use `u.flow` for camera travel. `u.beat` gets phase
corrections from the beat tracker, so motion driven by it stutters.

## Palettes

The **Show** tab's *Look* section picks a global colour palette —
WLED-style named gradients (`rainbow`, `party`, `ocean`, `forest`,
`sunset`, `lava`, `fire`, `ice`, `breeze`, `cyber`, `magenta`, `coral`,
`autumn`, `pastel`, `smoke`, `halloween`, `rift`, `gold`). Every scene's
`palette()` call samples a 256-entry gradient LUT uploaded once to the GPU,
so the whole show — scenes, dancer glow, and text — restyles instantly;
the per-cut hue still rotates through whichever palette is active.
Palettes are defined as colour stops in `src/palettes.rs` — add a line
there and it appears in the dropdown.

## Visual effects

A post effect transforms the whole frame — scene and dancer — chosen in the
panel or with `X`: **Mirror X**, **Mirror Y**, **Quad mirror** or **Kaleido
×6 / ×8**. The **Strength** slider blends the transform in (at 50% a mirror
sits over the plain frame). **Auto** picks a fresh effect on every scene cut
(Mirror Y stays manual-only — an upside-down dancer reads as a glitch).
It's applied in `present.wgsl` from `u.fx`, so it needs no scene support and
combines with everything (a mirrored dancer in canon is five dancers).

## Output to other screens (NDI)

The **Show** tab's *Network output* section sends the finished frame — scene,
dancer, post FX, text, blackout — as an **NDI** source on the LAN. Enable it,
pick a source name, and it shows up in:

- **OBS** (add an *NDI Source*, pick the Trippin sender — then stream, record,
  projector fullscreen, or VirtualCam it);
- Resolume, vMix, MadMapper, NDI Studio Monitor, or another machine running
  OBS — NDI crosses the network, so a second laptop can do the displaying.

It needs the free **NDI runtime**: `winget install NDI.NDIRuntime` (Windows)
or NDI Tools on macOS — Trippin loads it dynamically, so the app still works
fine without it (the panel just shows the error). Resolution (720p/1080p/4K)
and a 30/60 fps cap are independent of the window size; the render loop never
blocks on the network — frames drop rather than stall.

`trippin --ndi-monitor [name]` lists sources and counts frames from the first
match — a quick "is it on the wire?" check with no other tools needed.

## Timeline

The **Timeline** tab is a Clipchamp-style cue sheet for a track: load a song
(`--song`, the Load button, or dropping an mp3 / flac / wav / m4a / ogg on
either window) and it's decoded and analysed offline — the strip shows the
waveform, a beat/bar grid and a playhead. The grid is anchored to the
musical downbeat (bass-energy voting across the four beat slots), so bar
boundaries and cue transitions land on the "one", not just on beats.

- **Play** (`T`) plays the track through the speakers *and* the analyser, so
  the visuals react to the song itself; live input resumes when it ends or
  you stop it. Click/drag empty strip space to scrub the edit cursor.
- **Cues** are dragged from the editor's left palette onto the strip,
  snapped to quarter-beats (the `snap` toggle), edited below, or
  right-clicked to delete. A cue can cut to a scene, step scenes, switch
  modes, toggle the dancer / canon / blackout, pick a routine or look,
  set the post effect, or switch the global colour **palette** — palette
  cues latch until the next one, restyling every scene/text/dancer glow.
- **Text** cards live on the two bottom lanes: drag a card onto a lane,
  then drag it between `text 1` / `text 2` to move lanes. Alongside the
  look (neon/fire/wave/glitch/pulse/chrome) each card gets an
  editor-style entrance — fade, rise, drop, slide, zoom, or type
  (typewriter reveal with caret).
- **Record** (`G`) arms recording: while the timeline is playing, every
  hotkey and panel action lands on the strip as a cue — perform the show
  once, then save.
- **Save** writes `timelines/<name>.json` next to `trippin.json` (dropping a
  `.json` back on the window reloads it).
- **Follow live** is the adventurous bit: it correlates the room's live
  audio onset envelope against the track's stored envelope, locks on when
  the same song is playing in the room and fires the cues at the matching
  position — a pre-programmed show that follows the DJ's deck.
- **✦ AI show…** (editor toolbar) writes the cue list for you. Trippin
  analyses each clip locally — per-bar energy, onset density, a vocal
  likelihood, >5 kHz "air" — and segments the track into labelled ~4-bar
  phrase blocks (intro / groove / build / drop / peak / breakdown / outro).
  The model directs the show block by block (scene, dancer, routine, look,
  fx, palette, text per phrase), and Trippin expands that plan into cue blocks —
  enforcing variety itself (a scene can't run longer than ~12 bars, dancer
  routines rotate, `void` only ever plays a song out). BYOAI: Anthropic,
  OpenAI, Gemini, or any OpenAI-compatible endpoint (Groq, Mistral,
  Ollama…); the key lives in `trippin.json` or the provider's usual env var
  (`ANTHROPIC_API_KEY` etc.). Nothing but the feature summary leaves the
  machine — no audio is uploaded. Preview the summary with
  `--analyze <file>`, or run the whole build without the editor with
  `--ai-build <file>` (prints every cue).

## Silhouette dancers

Trippin ships with a library of female dancer silhouettes in the style of Bond
title sequences and 90s music videos, drawn over any scene. Auto-pilot runs it
by itself. On each scene cut it decides whether she's on screen (always during
breakdowns), picks the look (mostly the classic black shadow; neon, colour fill,
or strobe when the track is driving), goes to a three-dancer canon on
high-energy sections, and swaps to a clip whose energy suits the track:
graceful moves for breakdowns, salsa and grooves for drops.

Each clip is a loop of 8-bit masks in `dancers/<name>/` with a `clip.json`
giving its beat count and energy. Frame 0 lands on the downbeat, and playback
is stretched to the live BPM, switching to half or double time automatically
so the moves never look sped up or slowed down.

**The built-in library** is real dancers cut out of free Pixabay stock
footage by `tools/stock_dancer.py` (see `dancers/CREDITS.md` for sources).
It mats silhouette clips by brightness or colour, searches for the stretch
whose end pose best matches its start while she's actually moving, crossfades
the seam and loops it over a whole number of bars. The shipped clips run
8–16 beats at a calm-to-club energy range.

Add a clip from any silhouette-style footage:

```
py -m pip install -r tools/requirements.txt imageio-ffmpeg
py tools/stock_dancer.py clip.mp4 --name my_clip --matte dark    # dark dancer on bright bg
py tools/contact_sheet.py my_clip sheet.png                      # eyeball the loop
```

`--matte light` suits a bright dancer on black, `--matte green` a coloured
one, `--fill` fills glow-outline footage into a solid body, and `--beats`
sets the loop lengths to try (default 16/12/8).

A procedural library is still available: `tools/choreo.py` choreographs
shadow-dance routines on a real CMU mocap skeleton with IK-planted feet, and
`tools/mocap_dancer.py` renders them (or any BVH take) as curvy silhouettes
with simulated hair:

```
py tools/build_dancer_library.py             # downloads the takes, renders all clips
py tools/mocap_dancer.py take.bvh --name x --view three-quarter   # one clip from any BVH
```

To add dances, edit `LIBRARY` in `build_dancer_library.py`.
`cmu-mocap-index-text.txt` in github.com/una-dinosauria/cmu-mocap lists every
available take.

**Your own footage:** `py tools/roto.py <video> --name <clip> --start <downbeat s> --beats 8 --bpm <bpm>`
cuts a person out of a video. `--matte ai` (the default) uses rembg and needs
`py -m pip install rembg`; this path is untested so far. `--matte luma --invert`
suits a dark figure against a bright backdrop, and `--matte chroma` suits
green screen.

Motion data credit: *The data used in this project was obtained from
mocap.cs.cmu.edu. The database was created with funding from NSF EIA-0196217.*

## Layout

```
src/audio.rs     cpal capture (WASAPI loopback) + FFT analysis, onset/tempo/phase tracking
src/sysaudio.rs  macOS system audio via ScreenCaptureKit (the OBS desktop-audio API)
src/director.rs  auto-pilot: scene cuts every 8/16 bars or on a drop, intensity, palette
src/render.rs    wgpu: ping-pong HDR feedback targets, per-scene pipelines, hot reload
src/dancer.rs    dancer clips: background loading, beat-locked frame timing, auto half/double time
src/main.rs      winit app, keys, per-frame uniforms
shaders/dancer.wgsl      silhouette styles + canon, premultiplied-alpha over the scene
tools/mocap_dancer.py    BVH mocap -> female silhouette clip (loop finding, body loft, hair sim)
tools/build_dancer_library.py  downloads CMU dance takes and renders the built-in library
tools/roto.py            video -> silhouette clip (rembg / luma / chroma matte, via ffmpeg)
tools/contact_sheet.py   quick preview strip of a clip
shaders/common.wgsl      uniforms + helpers, prepended to every shader
shaders/present.wgsl     post: chromatic aberration, bloom, ACES / AgX tonemap, vignette, flash, grain
shaders/bloom.wgsl       13-tap Karis downsample / tent upsample chain (embedded, not hot-reloaded)
src/gfx.rs               baked 64³ noise volume + blue noise, bloom chain
src/snap.rs              --snap headless render + bench
shaders/scenes/*.wgsl    one file per scene; new files are picked up live
```

### Writing a scene
`cargo run -- --check-shaders` validates every shader with naga and exits — no
window, no GPU. Add `shaders/scenes/<name>.wgsl` with `@fragment fn fs_main(in: VsOut) -> @location(0) vec4<f32>`.
Everything in `common.wgsl` is available: `u.bass/mid/high/energy`, `u.kick`,
`u.onset`, `u.beat` (beat position), `u.beat_phase`, `u.bar_phase`, `u.build`,
`u.intensity`, `u.hue`, `u.seed`, `u.flash`, `spec(x)`, `prev(uv)` (last frame,
for feedback), `palette(t)`, `fbm`, `noise`, `rot`. Output is HDR, and the present
pass tonemaps it. Save the file to reload it. A compile error is printed and the
last good version keeps running.

Header tags go in the first 8 lines of a scene:
- `// @heavy` gates the scene to dGPU and Apple Silicon.
- `// @bloom 0.7` runs the bloom chain for that scene (0 or absent means no
  bloom passes at all).
- `// @tonemap agx` uses AgX (with a punchy look) instead of ACES.

Older scenes carry none of these, so they render exactly as before.

The 2026 helpers are:
- `tnoise(p)`: the four channels of a tileable 64³ noise volume, one fetch.
  R is Perlin-Worley, G is Worley fbm, B is smooth Perlin and A is finer
  Worley. The measured value ranges are in common.wgsl.
- `bluen(frag)`: per-frame blue noise for dithering ray-march starts.
- `fresnel`, `ggx`, and `cam_ray(p, ro, ta, roll, focal)`, which handles the
  y-down flip.

## Roadmap
1. **Milestone 1 (done):** audio analysis, 5 shader scenes, feedback, auto-pilot, fullscreen.
2. **Milestone 2: AI style layer.** A Python sidecar (StreamDiffusion / SD-Turbo,
   TensorRT on the RTX 5070 Ti) restyles the shader frame with a text prompt
   ("voodoo mask, neon, smoke"). Beat and energy drive denoise strength and prompt
   blending. Frames go over shared memory.
3. ~~Silhouette dancer layer~~ (done). Next: a live webcam silhouette of the DJ.
4. Control panel (egui): scene playlist, prompt presets, sensitivity, output select.
5. Outputs: Spout / NDI to Resolume, and recording to MP4 for social clips.
6. Deck awareness: Ableton Link, and track metadata / beatgrids from Rekordbox and
   Serato (reusing BeatDis readers) so phrase changes line up with the actual track.
