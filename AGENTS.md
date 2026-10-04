# AGENTS.md: working on Trippin

This file is for AI coding agents (Claude Code, Devin, and others) and for
the humans working with them. It holds the product knowledge and house rules
that aren't obvious from the code. More than one agent works in this repo, so
read **Working together** before you start. Update this file when you learn
something durable.

---

## 1. The product

Trippin is **live, music-reactive visuals for DJ sets**. It's written in
Rust with wgpu, and the visuals are WGSL fullscreen fragment shaders. It
listens to the system audio (WASAPI loopback on Windows, ScreenCaptureKit on
macOS) or a picked input device, and analyses it:

- band levels, onsets and kicks;
- tempo and the beat/bar phase;
- groove, which drives the beats-versus-breakdown mode.

An auto-pilot cuts between about 130 scenes on phrase boundaries and drops,
and strobes detected drum fills — a burst of onsets well above the section's
per-beat onset-density baseline, capped at a bar. When the fill collapses the
new section lands, so Auto can cut there even mid-bar (`director.rs`'s
`update_fill`; cuts ride `cut_on_drops` and a 2-bar minimum scene length,
fills never strobe on tunnel/flight scenes or in Manual mode). The bar grid
is a backstop, not the score: Auto also cuts off-grid on section events —
the breakdown detector committing (covers vocal breaks too), a sustained
energy surge (chorus/second-drop with no breakdown; fast vs slow energy EMA
in `update_events`), and the vocal-break proxy (bass thin, mids hot, 5
beats). Event cuts share a gap (1 bar in-scene + 4 beats since the last
cut), ride `cut_on_drops`, and stay off in Static and Manual. The queued
next scene is refit to the mood every frame (`repick_for_mood` in main.rs:
calm picks low-energy scenes, hot picks high-energy; random order only, and
never an operator's "play next").
Over the scenes it draws a beat-locked silhouette dancer, text, and stream
overlays.

- **Audience:** streamers (Twitch/YouTube/TikTok DJs), home DJs and mobile
  DJs. **Not** festival VJs. Features are prioritised for that audience:
  one-click OBS output, branding, now playing, and clips for social media.
- **Visual north star: Synesthesia.** Modern, photoreal-to-abstract GPU
  scenes that *move with the music*. Retro flat demoscene looks are the
  thing we're moving away from.
- **Performance floor: an RTX 3060 or an Apple M2 at 60 fps.** Rule of
  thumb: ≤1.3 ms/frame at 1080p on the dev RTX 5070 Ti means about 60 fps on
  an M2 at full resolution. Raymarched scenes carry `// @heavy`, which gates
  them to stronger GPUs.
- **Platforms:** Windows is the dev machine. The owner tests on an M2 Mac.
  macOS can't be cross-compiled from Windows (ring and bundled SQLite need
  the Apple SDK), so write `cfg(target_os = "macos")` code with extra care
  and say it hasn't been compiled yet.

## 2. The owner's taste (hard-won feedback: follow it)

- **Audio reactivity is motion, not flashing.** The music must change
  **shape** (morphs, displacement, geometry), **direction** (motion that
  reverses or swings with phrases) and **energy** (speed and complexity
  rising with the track). Brightness is the least important channel. Every
  new or reworked scene needs at least one audio→shape and one
  audio→motion mapping.
- **Architecture never changes shape.** Buildings, walls, towers, bar
  baselines and floors stay put. Shape reactions belong on abstract objects,
  lights, screens and displays. (Past bugs: skyscraper tops bouncing, and
  megastructure walls popping in and out.)
- **Motion must be smooth.**
  - Never feed raw audio into a motion integrator.
  - Never drive a pose with `u.bar_phase`: it snaps back every bar.
  - Never let a count (such as the number of beams) follow raw energy. It
    re-spaces everything and jerks. Keep the count fixed and fade the edges.
  - For travel and rotation use `u.flow` or `u.clock4` (the smooth energy
    clocks), and phrase-length sines for direction swings.
- **No beat-synced flashes in tunnels or flight scenes.** They read as
  "jerky".
- **Add scenes; don't replace them.** Removing or reworking a scene is the
  owner's call.
- **Removed or disliked scenes; don't bring them back:** event_horizon,
  volcano and wave_lines. cymatics was "meh" and got reworked, and
  chrome_spheres must not fly at the camera.
- **Lasers read as beams cutting through fog-machine smoke:** thin
  saturated cores, and patchy visibility along the beam where the smoke is.
  Laser scenes run *shows*: a new formation every 4 bars, morphing in over a
  beat.
- **Look before you claim it's done.** Render the scene (`--snap`) and look
  at the PNG. Many shader bugs only show up visually: upside-down worlds,
  seams, grey veils.

## 3. Working together (several agents in one repo)

- **Branches:** work on a feature branch (`feat/<topic>`). Never commit
  straight to `master`. Before you start, `git fetch` and rebase or merge the
  latest; another agent may have pushed since you last looked.
- **Don't** force-push shared branches, rewrite pushed history, or delete
  someone else's branch.
- **A new Unity show needs a new engine release.** `engine.rs::ASSET` pins a zip; an installed player that predates the show ignores its name (`ShowManager` keeps the current show), so Trippin would cut to `unity_<x>` and display a different show. Add the show in `StageBuilder`, a stub `shaders/scenes/unity_<x>.wgsl`, then publish `unity-engine-vN` (unity/README.md) before it ships.
- **`DropDirector.Impact` steps 0→1 in one frame.** Geometry or brightness driven straight from it pops at the drop (aurora measured a frame change of 33 against a 0.34 median). Give the show its own ~0.25 s attack and instant release (`AuroraShow._impS`, `1-exp(-12 dt)`), and keep the peak flare modest. `Tension` is already an integrator and needs no smoothing. Also check in a recording that a build's contraction is actually visible on screen (prism's was cancelled by the camera creeping in), and that a calm boost isn't cancelled by the tension dim (a breakdown is exactly when Tension is high).
- **Unity show checks that only a recording catches** (found on the M2 for basscore and pillars): a planar object (a disc, a floor plate) needs a camera well above it or it reads as a thin ellipse behind the near props; event pulses launched from kicks must take over the most-faded slot when all slots are busy, or one slow ring fills the slots and nothing reads; a field of beams needs a fade by horizontal distance from the camera (14 m to 5 m), or an orbit that passes close fills the lens with one blurred slab. The synthetic feed's `hits4[0]` is `exp(-7*beat_phase)`, so kick thresholds always look right there and must be checked live.
- **A Unity show must react on real music, not just the synthetic feed.** The owner judged lightstorm "not very reactive" on a real track: `pres4` and `clock4` move slowly there, so a show that reads only those looks inert. Give every show at least one eased `lvl4` or `hits4` mapping onto *shape* (`Eased.Follow`: attack ~14-20/s, release 3-4/s, in `Scripts/Eased.cs`), never onto brightness and never into a motion integrator.
- **Kit shows** (`Scripts/Kit.cs`: `KitShow`, `Rx`, `BeamPool`, `GlowPool`, `HazeSet`, `CamRig`) are the fast way to add a Unity show: subclass `KitShow`, implement `Build()` and `Frame()`, add one line to the `kitShows` array in `StageBuilder`, add a stub `shaders/scenes/unity_<x>.wgsl` (its first sentence feeds the AI builder's flight detection: say "flying"/"flight" for flight scenes), and map `rx.*Fast`/`rx.kick`/`rx.Spec()` onto shape and `rx.Gain()` onto luminance. They were type-checked against a stub of the Unity API with `mcs` (no Unity in the cloud container) but not compiled or rendered in Unity: expect the M2 to find look and cost problems.
  - **Thin beam polylines read as amateur.** The owner called the first helix/knot/gyroscope ("extremely basic"), the pendulum wave, the wire terrain/ocean/tunnel and the sheet-based nebula "too basic" or "rubbish". Build curves as `TubeRibbon`s (`Tube.shader`: one mesh per curve, white-hot core + halo, spectrum carried along the length), surfaces as lit opaque meshes (`Land.shader`, with `Horizon.shader` behind), and big environments as full-screen passes (`Kit.Fullscreen` + `TrippinFull.hlsl`: nebula `DeepSpace`, tunnel `Corridor`, the radar disc `Scope`). Keep `BeamPool` for light shafts and fine accents only.
  - `Rx` pushes its eased signals as shader globals every frame (`_RxLvl`, `_RxMisc`, `_RxClk`, `RxSpec(x)` in `TrippinCommon.hlsl`), so shaders get the same smoothed vocabulary without per-material plumbing. Never use the raw `_TLvl`/`_TSpec` for shape.
  - Check shader syntax in the cloud container with `python3 tools/hlslcheck.py Assets/Trippin/Shaders/<x>.shader` (needs `apt install glslang-tools`; it stubs the Unity includes, so it catches typos, not Unity-specific problems). It cannot render: Land/DeepSpace/Corridor/Scope looks were previewed with numpy ports, so the M2 still has to confirm composition and cost.
- **Unity instanced draws need instancing on the material asset at build
  time** (`StageBuilder`: `mat.enableInstancing = true`). Enabling it only
  on a runtime copy lets the build strip the shader's instancing variant
  ("after built-in stripping: 1" in Editor.log), and
  `Graphics.RenderMeshInstanced` then draws nothing, without any error.
- **One agent per checkout.** Switching branches in a folder another agent
  is using swaps the files under it. A Unity build that was running then
  silently compiles the other branch's code (the v4 Mac engine almost
  shipped without its new shows this way). Give each agent its own
  `git worktree add ../Trippin-<topic> <branch>`, and build releases from a
  worktree. The first Unity build in a new worktree re-imports the project
  (several minutes).
- **Small commits.** Each commit gets a descriptive message that says what
  changed and why. Push when a piece works, so the other agent can see it.
- **Merging to master:** `git merge --no-ff` with a `Merge feat/<x>: …`
  summary, or a PR. Only when the owner asks.
- **CI costs the owner money.** `release.yml` runs only on `v*` tags and
  manual dispatch; don't add push/PR triggers back. One release = one tag
  push = one run.
- **Releases:** bump `version` in `Cargo.toml` on master, commit
  "Bump version to X.Y.Z", then push an annotated tag `vX.Y.Z`. Pushing a
  `v*` tag triggers `.github/workflows/release.yml`, which builds the
  Windows installer and publishes the release. Only when asked.
- **Never commit:** `trippin.json` (user settings), `snaps/`, `target/`,
  recordings, or API keys.
- **If your change is a durable rule or gotcha, add it here** in the same
  commit, so the next agent doesn't relearn it.

## 4. Build, run, verify

```
cargo build --release
cargo test --release                      # unit tests: audio, now-playing parsers, Spout round-trip, overlays
cargo run --release -- --check-shaders    # naga-validates every shader, no GPU/window
cargo run --release -- --snap laser_show,stage_rig --snap-size 1920x1080   # headless PNGs to snaps/ + ms/frame
cargo run --release -- --snap all --snap-bench 120   # perf table in snaps/bench.tsv
cargo run --release -- --snap x --snap-calm 1        # preview breakdown (no drums) mode
cargo run --release -- --snap aurora --snap-dancer comic --snap-clip stock_disco   # a dancer look over a scene (name or index; default clip = first)
cargo run --release -- --nowplaying       # prints what each now-playing source sees
cargo run --release -- --list-devices     # capture devices (names for --device / the Audio in picker)
cargo run --release -- --probe-audio [name]   # capture ~6 s, print band peaks + BPM; exits nonzero on silence
cargo run --release -- --beats track.flac     # onset grid vs Beat This! grid + timings (TRIPPIN_BEATS_DEBUG=1: per-30 s tempo)
cargo run --release -- --spout-grab <name> out.png   # receive one Spout frame (Windows)
cargo run --release -- --ndi-monitor [name]
cargo run --release -- --list-midi          # MIDI input ports (pad/key controllers)
```

- **Dancer looks** are `dancer::STYLES` (shadow, neon, strobe, comic, wire); the
  index is stored in settings, timeline cues, AI plans and the iOS remote, so
  only ever append. `--snap-dancer <look>` renders one over a scene, which is how
  to check a new look (dancer.wgsl is full-screen, so keep per-pixel mask taps
  behind the `outside_sprite` early-out).
- In the Linux cloud container the app can't be built as-is: `src/midi.rs`
  fails on midir's ALSA types (`?` needs `Sync`). Patch it locally with
  `.map_err(|e| anyhow::anyhow!(e.to_string()))?` after `input.connect(...)` to
  build, run `--check-shaders` and render with `--snap` on llvmpipe (install
  `libasound2-dev pkg-config libudev-dev mesa-vulkan-drivers libvulkan1`; about
  4 minutes a 640x360 frame for the comic look), and **don't commit the patch**.
  `overlay::tests::overlay_images_render` also fails there if `target/` doesn't
  exist (it writes PNGs into it).
- If `trippin.exe` is running, it locks `target/release`. Build into another
  directory: `--target-dir target/verify`.
- `--snap` drives scenes with a synthetic 126 BPM groove at 60 fps, and time
  runs continuously. `--snap-at 5,5.05,5.1,…` gives a frame sequence for
  GIFs and videos.
- When testing the GUI app, run it from a scratch folder with its own
  `trippin.json`. It loads `./trippin.json` first, else
  `%APPDATA%\Trippin\trippin.json`, which is the **owner's real settings,
  so don't clobber them**.
- **Key injection:** `SendKeys` sometimes misses the winit window.
  `PostMessage` WM_KEYDOWN/UP to the hwnd is reliable.
- The agent Bash tool's heredocs expand `\n` into real newlines, which
  silently breaks Rust string literals edited through `python - <<EOF`.
  Write edit scripts to a file (or use the Edit tool) instead.
- The dev box has no ffmpeg on PATH. imageio-ffmpeg's binary (Python) works
  via the `ffmpeg_path` setting.

## 5. Architecture map

| Path | What |
|---|---|
| `src/main.rs` | The winit app and CLI flags. `render_loop` runs on **its own thread** (Windows' modal move loop would freeze it otherwise). The event thread runs input and the egui panel. They share `Shared` (a `Mutex<Settings>`, a `Mutex<Status>`, and atomics) and talk over `mpsc::Msg`. |
| `src/audio.rs`, `src/sysaudio.rs` | Capture (CPAL devices; the macOS system-output tap lives in `sysaudio.rs`), FFT, onsets and kicks (level-independent: flux > mean×1.8), tempo PLL, groove, `calm` (breakdown), the four-band vocabulary, the triggered waveform, and the neural downbeat check (`nn_*`: a 15 s window every 5 s to a `beat-nn` worker thread). |
| `src/beats.rs` | Beat This! (`beat-this` crate, ONNX via pure-Rust `rten`, as in BeatDis): model download to `<data dir>/models` (SHA-checked, from the public `BeatDis-models` release), detection with sub-frame refine, the constant-tempo grid fit, and the per-song grid cache (`<data dir>/beatcache`). |
| `src/director.rs` | Auto-pilot: phrase cuts (backstop), drop cuts, off-grid section-event cuts (breakdown entry, energy surge, vocal break), drum-fill strobes + sub-bar cuts, intensity, and the beats/breakdown modes. |
| `src/render.rs` | wgpu. Ping-pong Rgba16Float feedback targets, per-scene pipelines, hot reload, and the `Uniforms` struct (**must match `U` in `shaders/common.wgsl`**). |
| `src/gfx.rs`, `shaders/bloom.wgsl` | The baked 64³ noise volume and blue noise, and the bloom chain. |
| `shaders/common.wgsl` | Uniforms and helpers, prepended to every shader. |
| `shaders/present.wgsl` | Post-processing: FX (mirror/kaleido), bloom mix (capped at 12%), ACES/AgX, vignette, grain, and transparent-mode alpha. |
| `shaders/scenes/*.wgsl` | One file per scene, auto-discovered and hot-reloaded. |
| `src/dancer.rs`, `shaders/dancer.wgsl`, `dancers/` | Silhouette clips (PNG mask sequences), beat-locked. There's an energy cap for slow tempos. |
| `src/text.rs`, `shaders/text.wgsl` | Timeline text cues (ab_glyph masks). |
| `src/overlay.rs`, `shaders/overlay.wgsl` | Stream overlays: the now-playing card, branding and the ticker. They're drawn on the CPU into RGBA images when their content changes, and composited in the present pass. |
| `src/nowplaying.rs` | Track detection, with a worker thread polling every 1 s. |
| `src/output.rs` | The output tap. It re-runs present at the output size, reads it back asynchronously, and feeds the sinks (NDI, Spout, recorder). |
| `src/ndi.rs`, `src/spout.rs` | NDI (runtime loaded dynamically), and a native Spout2 sender (D3D11 shared texture plus the Spout shared-memory registry). |
| `src/rec.rs` | Clip recording: the ffmpeg replay buffer and set recording. |
| `src/timeline.rs`, `src/song.rs`, `src/editor.rs`, `src/ai.rs` | The timeline show editor (F2), song playback, and the AI show builder (local analysis → prompt → plan → `expand_plan` rules → cues). |
| `src/panel.rs` | The egui control panel. Tabs: Perform, Dancer & FX, Stream, Timeline, Keys, Settings. App-wide preferences (audio in, latency, director rules, AI provider/key) live on **Settings** (`settings_tab`), not in collapsibles on other pages or in the timeline editor. |
| `src/config.rs` | `Settings` (serde, `#[serde(default)]`), actions and hotkeys, and `data_dir()`. |
| `src/midi.rs` | MIDI input (midir): one port, note-ons become `Action`s. |
| `src/remote.rs` | LAN remote for the iOS companion app: a WebSocket JSON server (TCP 9138, Bonjour `_trippin._tcp`, PIN-gated) — protocol at the top of the file, details in §8. |
| `ios/TrippinRemote/` | The iOS/iPadOS remote app (SwiftUI, iOS 17+): Bonjour discovery, PIN pairing in the Keychain, pads, scene grid with thumbnails, look/FX, dancer and transport pages, plus a built-in `DemoServer` for use without a rig (and for App Review). The `.xcodeproj` is generated: run `xcodegen` in that folder (it's git-ignored). Speaks the `remote.rs` protocol; UI tests in `UITests/`. See its README. |
| `src/osc.rs` | OSC UDP input (9139) for TouchOSC/Lemur — maps addresses onto the same `RemoteCmd`s as the app. |
| `src/snap.rs` | Headless snapshot and benchmark rendering. |
| `src/engine.rs`, `src/link.rs`, `unity/` | Unity engine (shows `unity_stage`, `unity_crystals`, `unity_flow`, `unity_leviathan`, `unity_sculpture`, `unity_colossus`, `unity_tidal_cathedral`, `unity_lightstorm`, `unity_prism`, `unity_aurora`, `unity_basscore`, `unity_pillars`, `unity_orbit_foundry`, `unity_helix`, `unity_tesseract`, `unity_polyhedra`, `unity_knot`, `unity_gyroscope`, `unity_lattice`, `unity_pendulum`, `unity_spectrum`, `unity_eclipse`, `unity_lightrain`, `unity_warp`, `unity_ledwall`, `unity_fountain`, `unity_galaxy`, `unity_nebula`, `unity_terrain`, `unity_ocean`, `unity_chladni`, `unity_tunnel`, `unity_orb`, `unity_radar`; any `unity_*` scene is gated on live frames and hidden from the AI builder). `engine.rs` launches the player headless and supervises it; frames come back through a memory-mapped file (seqlock, top row first) into `gfx::Statics::ext` (binding 8 `ext_tex`); `link.rs` sends the show state over UDP. Cross-platform, nothing to start by hand. See `unity/README.md`. |
| `tools/*.py` | Offline pipelines: mocap and stock video to dancer clips, and so on. |

## 6. Writing a scene

- **The entry point:** `shaders/scenes/<name>.wgsl` with
  `@fragment fn fs_main(in: VsOut) -> @location(0) vec4<f32>`. The output is
  HDR, and present tonemaps it.
- **Header tags** go in the first 8 lines: `// @heavy`, `// @bloom 0.7` and
  `// @tonemap agx`.
- **The audio vocabulary** (see `common.wgsl`):
  - `u.lvl4`, `u.hits4` and `u.pres4` are loudness, transients and slow
    presence, per band: bass, mid, mid-high and high.
  - `u.clock4` are the energy clocks (in beats), for smooth speed that
    surges on drops.
  - `u.flow` is a smooth beat clock. `u.calm` goes from 0 while the beats
    play to 1 in a breakdown.
  - `u.intensity` is the overall drive. There's also `spec(x)` (the
    spectrum), `wave(x)` (the waveform), and the helpers `bpm_sin`,
    `bpm_tri`, `beat_pulse` (calm-aware), `random_on_beat` and
    `toggle_on_beat`.
- **Other helpers:**
  - `palette(t)` is the global palette. Square it for saturation.
  - For noise: `hash21` (PCG), `noise`, `fbm`, `tnoise` (the 64³ volume) and
    `bluen`.
  - For shading and cameras: `fresnel`, `ggx` and `cam_ray`.
  - `smoke2d` and `laser_line` build laser and fog scenes.
  - `prev(uv)` reads last frame, for feedback.
- **Colour:** use a dark base and a saturated palette. Keep feedback gain
  around 0.15–0.2; above about 0.75 it blows out. Keep emissive surfaces
  below ~1.0 under AgX, or they go pastel.
- **Perf:** at most 1.3 ms at 1080p on the 5070 Ti. Measure it with
  `--snap <name> --snap-bench 120`.

### WGSL and shader gotchas

- `centred(uv)` has **+y pointing down**. Negate it when building ray
  directions (`rd.y = -p.y…`), or the world renders upside down.
- There's no swizzle assignment (`v.xy = …` fails), no ternary (use
  `select`), and `from` is a reserved word.
- `pow(x, y)` is undefined for negative x, so square by hand. `atan2(0,0)`
  is undefined, so use `angle()`.
- **Polar seams:** `angle(p)/TAU` wraps at ±π. Mirror the lookup domain.
- A per-pixel dynamically indexed `array` spills to local memory (1.4 ms
  became 11 ms). Recompute instead, or use `const` arrays with early-outs.
- Float `fract(sin(dot…))` hashes go blocky at large coordinates. Use the
  PCG `hash21`.
- `tnoise` channel stats are skewed (R p50 ≈ 0.22). Threshold against the
  measured ranges in `common.wgsl`, not 0.5.
- **AgX lifts blacks** about 3× compared with ACES. That's why transparent
  mode forces ACES.
- Uniform struct arrays need a 16-byte stride. Any change to `Uniforms`
  must be mirrored in `common.wgsl`, and in every constructor: main.rs,
  `render.rs::thumbnail` and `snap.rs`.
- One NaN lives forever in feedback. `prev()` and present sanitise with
  `finite()`.
- A roll (`rot` on screen coordinates) tilts the horizon. Use yaw on
  `rd.xz` for level scenes.
- Domain-repeated cities and facades: evaluate the current cell only, and
  clamp the march step.

## 7. Rust and app gotchas

- **Never take the same `Mutex` twice in one statement.** Temporaries live
  to the end of the statement, so
  `Status { a: lock(&m).x, b: lock(&m).y }` deadlocks the render thread.
  Take one guard first.
- **Lock order:** never hold the `timeline` lock while locking `settings`.
- Settings changed on the render thread must set `shared.dirty` to be
  saved. Timeline cue playback must **not** persist (`fire_cue(…, persist:
  false)`); show state must never leak into `trippin.json`.
- New `Settings` fields need a `Default` value. `#[serde(default)]` keeps
  old files loading. Enums that can lose variants use `#[serde(other)]` on
  the last variant.
- **trippin.json must survive newer builds** (`Settings::parse_lenient`): a
  strict parse once dropped the whole file over one unknown hotkey action
  written by a newer dev build, so every setting (the Unity link included)
  reset on each launch of the installed app. Unknown actions in `keys` /
  `midi_notes` are filtered and any other field that doesn't fit is
  skipped on its own. Dev builds run from the repo use the owner's real
  settings: run them from a scratch folder (§4).
- `Settings.palette` can be `"auto"` — a pseudo-palette, not a gradient.
  `render_loop` resolves it per frame through `palettes::Auto` (music mood →
  a named palette, with hysteresis so the LUT can't strobe). Pickers and
  validators must use `palettes::all_names()` / `is_valid()`, never
  `names()` alone, or the option silently disappears.
- **Audio streams die mid-set and must be rebuilt, not just logged.** A
  WASAPI loopback can strand silently (default-device change, session
  invalidation) — the callback keeps running but delivers nothing, or the
  error callback fires once and the stream is over. `AudioEngine.dead`
  flags fatal `cpal::ErrorKind`s (`Xrun`/`DeviceChanged` are *not* fatal:
  glitches and automatic reroutes); the analyser's `recv_timeout` feeds
  silence on a stall so features decay instead of freezing; and a
  render-loop watchdog rebuilds the engine on `dead`, a stale `phase_at`,
  or ~15 s of silence, with backoff so a genuinely quiet/unavailable source
  isn't re-opened every frame. Never run that watchdog while a timeline
  show is playing — a paused song's silence is legitimate.
- `trippin.json` can carry a UTF-8 BOM (Notepad, PowerShell 5), and loading
  strips it. Do the same for any user-edited text files you read.
- Anything that re-installs global taps (see `rec.rs`) must drop the old
  instance **before** starting the new one.
- egui 0.36: `TexturesDelta` must be drained, not iterated. The text-field
  key sets live in both `panel.rs` and `editor.rs`.
- The Windows GUI build uses the `gui` feature for no console window. Don't
  pass `/SUBSYSTEM:WINDOWS` via `RUSTFLAGS`.
- Worker threads must always reply to their channel, even when they panic
  (use `catch_unwind`). A dropped reply wedged the editor.
- **Windows `accept` inherits non-blocking** (unlike Unix): a stream from
  a `set_nonblocking(true)` `TcpListener` arrives non-blocking, and
  `set_read_timeout` is a dead letter on it — `read` returns `WouldBlock`
  instantly. Call `set_nonblocking(false)` on every accepted stream (see
  `remote.rs::client`). The symptom is random `WouldBlock`/reset reads —
  easy to misread as a firewall.
- **Network tests bind `127.0.0.1`** (`Server::start_on`, `Osc::start_on`):
  binding `0.0.0.0` in a test pops the Windows firewall prompt and stalls
  the socket I/O until it's acknowledged. Same reason mDNS adverts are
  skipped on loopback binds.
- `f32::signum(+0.0)` is **1.0**, not 0.0. An ease like
  `v += rate*dt*(target - v).signum()` never rests: at `v == target` it
  steps up `rate*dt` then eases back — a two-frame judder (this made the
  branding logo wobble horizontally whenever the DJ name was off). Guard
  the at-rest case (see `overlay.rs::ease_vis`).
- **HTTP clients use the OS trust store** (`config::tls()`, ureq's
  `platform-verifier` feature). ureq's default bundled roots failed with
  `invalid certificate: UnknownIssuer` on the owner's M2, whose network
  inspects HTTPS. Every new `ureq::Agent` must set
  `.tls_config(crate::config::tls())`.
- The render thread is **not unwound on quit**: its locals' `Drop`s never
  run (macOS Cmd-Q doesn't even return from `run_app`). Process-level
  cleanup (killing the Unity player: `engine::shutdown`) goes in
  `ApplicationHandler::exiting`, with state the event thread can reach.
- Panel scene thumbnails are rendered once and disk-cached, except
  `unity_*` tiles (`panel.rs::live_thumb`): they re-render about every
  0.5 s from the engine's current frame and are never cached.
- The pointer over the visuals is hidden while fullscreen (synced in
  `about_to_wait`); the panel/editor windows keep theirs.
- **MIDI** (`midi.rs`): midir's callback thread posts `AppEvent::MidiNote`
  through an `EventLoopProxy` — the event loop type is
  `EventLoop<AppEvent>`, so `ApplicationHandler<AppEvent>::user_event`
  dispatches. `App::midi_note` is the single entry point: it serves
  MIDI-learn (Keys page `midi_learn`) or runs `apply` like a hotkey, so
  pad presses record as timeline cues too. Only note-on with velocity > 0
  counts (note-off / vel-0 is the pad release — acting on it doubles
  toggles). Bindings are `Settings::midi_notes` (action → note, any
  channel; one note = one action, learning steals it). WinMM has no
  unplug event: `about_to_wait` rescans `midi::ports()` every 3 s and
  drops a connection whose port vanished, so replugging recovers. midir's
  macOS backend is CoreMIDI — the code isn't cfg-gated, but it hasn't been
  compiled for macOS yet.
- **Beat This! grid fit** (`beats.rs::fit_grid`): the model sometimes emits
  *confident* beats at ~1.5x tempo plus dozens of junk downbeats through a
  breakdown. Counting those as beats skewed tempos by 2-3%, and a slightly
  wrong tempo spreads the downbeat vote over all four phases. So: refine
  beat times sub-frame (raw peaks are on a 20 ms grid), take the period
  from steady runs that agree with the median, number each beat against
  the previous accepted one and skip beats between grid lines, reject
  residual outliers and refit, and let only accepted beats' downbeats vote.
  Check changes with `--beats` on several tracks: real tempos come out as
  round numbers (125.00, 130.01, 140.87).
- **Live beat phase** (`audio.rs` `nn_vote`): the onset comb locks onto the
  strongest onsets, which in a lot of house are the off-beat bass/hats —
  measured with `--groove-test`, the live "one" was on the bar only 5-30%
  of the time. The neural window measures where real downbeats sit on the
  live grid (circular mean), shifts the phase and stores `nn_bias`, which
  `correct_phase` adds to the comb target so it doesn't drag the phase
  back. Then the downbeats vote the bar (decisive when >=4 agree at 85%).
  `--groove-test` prints `bar N` (live "one" vs the file grid; 0 = right),
  `TRIPPIN_NO_NN=1` compares without the check, `TRIPPIN_NN_DEBUG=1` logs
  each window. `main` caps rten at 2 threads (`RTEN_NUM_THREADS`); it
  barely scales past 4 and would otherwise take every core.
- `song::load` uses the Beat This! grid when `beats::ready()` (downloaded and
  `Settings::beat_model` on); downbeat agreement < 50% keeps its tempo but
  picks the bar by bass vote. `ai::build_show` returns re-detected grids
  (`ShowBuild::grids`) and the editor applies them before adding cues.
- **AI show builder** (`ai.rs::anthropic`): the default is
  `claude-sonnet-5-5`. Claude 5-family models reject a forced
  `tool_choice` (`tool`/`any`) with a 400, so `emit_plan` is offered with
  `auto` and the system prompt asks for the call. Thinking is always on
  and counts toward `max_tokens` (keep it ≥16k). Check `stop_reason`
  (`refusal`, `max_tokens`) before reading content. `output_config.effort`
  and `fallbacks: "default"` are sent only for the 5-family ids (the model
  and endpoint are user-editable), and the fallback only to
  api.anthropic.com. When bumping a default model, add the old id to the
  retired-id reset in `Settings::load`.
  - With `Settings::ai_web_search` the request also offers the
    `web_search` server tool (max 4 uses); a `pause_turn` stop is resumed
    by re-sending with the paused content appended as the assistant turn.
  - Show rules live in two places: the system prompt asks for them, and
    `expand_plan` enforces them (routine pace by block kind via
    `routine_energies`/`CALM_MAX`, neon look in sung calm blocks, drop
    impact top-up, title card fallback, mid-song `void` ignored). Change
    both together. Unknown routine names are replaced by pace, not dropped.
  - Analysis runs on the *timeline clip's* grid (`analyze_song(…, c.bpm,
    c.first_beat)`) so an editor-nudged grid still lines bars up with cues.
  - Structure (`find_boundaries`): novelty over 3-bar-median-filtered,
    z-scored features (energy, onsets, vocal, air, mid) + half the
    bar-to-bar jump, isolated fills boost the next bar, phrase grid voted
    with a bar-0 prior, snap only when the on-grid neighbour scores ≥60%,
    min 4 bars apart, then fill-led splits of long flat stretches. Section
    kinds are relative to the track's own median/p75. Check with
    `--analyze` (`TRIPPIN_AI_DEBUG=1` prints the novelty curve) on a flat
    house track (Krush – House Arrest) and an EDM one (D.O.D.).
  - Scene energy comes from `shaders/scene_energy.json`, generated by
    `--snap all --snap-energy` (motion ×2 + brightness + colour, ranked).
    **Re-run it when you add scenes** (unknown scenes default to 0.5).
    `scene_meta` lifts laser/festival rigs to ≥0.6 and flags tunnel/flight
    scenes, which never get strobe/flash fills (they stutter instead).
  - Plan dialect extras: `cuts` (in-block scene changes), `fill`
    (strobe/stutter/flash on fill bars), text `fx`/`size`/`at`/`seq`.
  - **Strobe** is `CueKind::Strobe`, not blackout: blackout eases `master`
    at ~3/s so sub-beat pulses never got dark. The strobe gate in the render
    loop is hard (no easing) and opens only while the live analyser's
    `onset` ≥ 0.45 (~70 ms per hit), so flashes follow the drums actually
    playing, not the bar grid. The AI places it over `fill_span` — the
    dense run of `ClipAnalysis::beat_onsets` around the fill bar, which can
    start mid-bar or reach into the bar before. `STROBE` is a static flag
    (reset in `apply_playhead` and `end_show`). Check with
    `--groove-test` + `TRIPPIN_GATE=from-to` (prints the gate per hop, `|`
    per beat) and `--analyze` (`fill_spans`). `TRIPPIN_CUE_TEST='{"Strobe":true}'`
    fires any cue at startup.
  - Dancer routines re-anchor their loop to the bar they were requested in
    (`Slot::pending_anchor`), so a routine switched in on a phrase starts
    from its first frame there.
- **Text effects** (`TextFx`, `text.wgsl`): punch/shake/strobe/bounce/
  shatter use the slot's former padding float (`TextSlotU::fx`), so the
  uniform layout didn't change. `TRIPPIN_TEXT_TEST="WORDS|fx"` fires one
  text cue at startup — screenshot the visuals window to look at an effect
  without playing audio.
- **macOS audio:** ScreenCaptureKit hears only the *system output mix*.
  DJ software routed straight to a controller's own interface (Serato → a
  Rane's USB card) never enters it — capture shows "no signal" while music
  plays, and the tap is flaky even when the audio does enter the mix.
  The fix is the controller's input device: `Settings::audio_in`
  (panel picker on the Settings tab, live-restarts the engine
  via the settings diff in `render_loop`), or `--device`/`--mic` per run.
  `audio_in` is a device-name substring resolved by `AudioEngine::start`,
  and a stale value falls back to the default tap rather than blocking
  startup. `Settings::load` runs before `AudioEngine::start` in `main` so
  the saved source applies at launch — keep that order. Debug capture
  with `--probe-audio [name]` (no window needed).
- egui layout rules the UI relies on (learned the hard way, review A1):
  - `ui.horizontal` children see the parent's `max_rect`, not the shrunk
    `cursor` — a `right_to_left` or `available_width()` inside one can
    overflow and re-widen `cursor.max.x`, pushing later rows under a
    `Panel::right`. Capture the needed width at parent scope first, or
    paint inside a bounded `allocate_exact_size` rect.
  - `ui.columns` inside a `ScrollArea` lets card content bleed under the
    neighbour column; use `ui.new_child(UiBuilder::max_rect(...))` columns
    and `ui.add_space(col_height)` to claim the row (see `stream_tab` /
    `dancer_fx_tab`).
  - In `egui::Grid`, every `ui.add` is a new cell — multi-widget form rows
    must be wrapped in one `scope_builder` horizontal cell (see `grow`),
    or the extra widgets become columns and stretch the grid.
  - Cards only bound their own frame, not content: an over-long label or
    path widens `min_rect` and the card escapes its column. Elide text
    (`ellipsize` / `elide_left` / `elide_mid`) rather than trusting clip.
  - No font glyphs for icons — the bundled fonts lack them (tofu). Paint
    checkboxes/ticks (`paint_check`), transport buttons (`t::tr_btn`), and
    the record dot (`t::rec_btn`) instead. `key_short` maps key names to
    words (`Space`, `Right`) for the same reason.
  - `wants_keyboard_input()` is true for ANY focused widget — a clicked
    pad/button counts, so "is the user typing?" gates must use
    `text_edit_focused()` (`EguiWin::wants_keyboard`). Otherwise egui
    swallows Space and re-triggers the focused widget instead of firing
    the bound hotkey.

## 8. Streaming features: how they work

- **Now playing** (`nowplaying.rs`):
  - **Sources:** the OS media session (Windows SMTC; on macOS, AppleScript
    for Spotify and Music, and only for apps that are running), Serato
    (4+: `Library/master.sqlite` `history_entry` where `played`=1, in
    `Application Support/Serato` / `%APPDATA%\Serato`, WAL-mode — watch the
    `-wal` mtime; ≤3.x: `_Serato_/History/Sessions/*.session` binary),
    VirtualDJ `tracklist.txt`, rekordbox `master.db`
    (decrypted SQLCipher), the Mixxx set log, and a text-file watcher.
  - **Auto** picks the source whose track changed most recently. History
    files don't count as a change at startup; live sources do.
  - `np_delay_s` is the debounce that skips headphone cue-ups.
  - It writes `nowplaying.txt` next to `trippin.json`.
  - **Not built yet:** mixer and on-air detection (needs MIDI or Pro DJ
    Link), Serato's Remote protocol, and a Traktor Icecast listener.
- **Output tap** (`output.rs`): renders at 720p, 1080p or 2160p at 30 or 60
  fps, and never blocks rendering (frames drop instead).
  - **Transparent mode** (`Uniforms::misc4.x`): scenes are skipped, and
    present writes premultiplied alpha. The worker un-premultiplies, because
    NDI and Spout expect straight alpha.
- **Spout** (`spout.rs`): the `SpoutSenderNames` registry (256-byte slots,
  sorted), a 280-byte `SharedTextureInfo` per sender, and the mutexes
  `<name>_mutex` and `<name>_SpoutAccessMutex`. There's no Syphon on macOS
  yet, so Mac users use NDI.
- **Recording** (`rec.rs`):
  - One ffmpeg encodes 2 s MPEG-TS segments, keyframe-aligned, with a
    constant frame rate locked to the wall clock.
  - Audio is tapped at every capture source (`rec::audio_in`), resampled to
    48 kHz stereo, and chunked on the same 2 s grid.
  - A save concatenates the segments (`-c copy` for 16:9, re-encoded for
    9:16 crop and fit). The concat lists need **absolute** paths.

## 8.5 LAN remote (iOS app + OSC)

- **WebSocket server** (`remote.rs`, `Settings::remote_*`, default off):
  TCP **9138** on all interfaces, Bonjour `_trippin._tcp` as "Trippin on
  <host>"; the Settings card shows `IP:port` as the manual fallback and
  the pairing PIN. `remote_on` with a blank PIN gets a fresh one on load —
  auth can't silently disable.
- **Protocol** (documented at the top of `remote.rs`): JSON text frames;
  the first must be `{"cmd":"hello","pin":…}` — a wrong PIN earns an `err`
  frame, a 500 ms penalty, and a close. `hello` replies with scene/clip/
  palette/action lists. Commands (`action`, `goto_scene`, `queue_next`,
  `show_clip`, `set`, `transport`, `thumb`) become `RemoteCmd`s posted to
  `AppEvent::Remote` → `App::remote_cmd`, which runs `Action`s through
  `apply()` so remote presses record into an armed timeline like hotkeys.
- **State pushes** run ~10 Hz from a pusher thread that serialises once
  and hands each client a copy over a bounded channel — a client whose
  queue stays full is dropped. `thumb` requests queue until
  `shared.thumbs` has `"scene:<name>"` (requested via `Msg::Thumb`), then
  ship as base64 PNG. No lock is held across socket I/O.
- **OSC** (`osc.rs`, `Settings::osc_*`, default off): UDP **9139**, the
  `/trippin/…` address table at the top of `osc.rs`, mapped onto the same
  `RemoteCmd`s. Buttons fire on press only — a falsy first arg is the
  release (the MIDI note-off lesson); `/set/*` and `/scene/goto|queue`
  read their arg verbatim.
- First enable pops the Windows firewall prompt once — expected.
- **The advertised address** (`remote::local_ip`, used for Bonjour and the
  Settings card) comes from the interface list, preferring 192.168/16,
  then 10/8 and 172.16/12, and skipping link-local 169.254 and Tailscale's
  100.64/10. Routing towards the mDNS multicast group picked Tailscale's
  self-assigned 169.254 adapter on the owner's PC, so phones were sent an
  unreachable IP.
- `tools/remote_test.html` is a browser harness for the same protocol —
  pads, scene grid, thumbs — for testing without the iOS app. The server
  serves it on a plain HTTP GET (no WS upgrade headers), so browsing to
  `http://<ip>:9138` is the no-install remote. `serve_page` peeks at the
  request without consuming it; when it answers, it must **drain the
  request bytes first** — closing a socket with unread inbound data RSTs
  it and the response can be lost.

## 9. Docs

- The user-facing docs are in `README.md`. Keep them current when you add
  features, hotkeys or flags. Media lives in `docs/media/`.
- When a hotkey is added, update `config.rs` (`Action::ALL`, `label`,
  `default_key`) and the README key table.
