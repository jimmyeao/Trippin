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

An auto-pilot cuts between about 130 scenes on phrase boundaries and drops.
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
- **Small commits.** Each commit gets a descriptive message that says what
  changed and why. Push when a piece works, so the other agent can see it.
- **Merging to master:** `git merge --no-ff` with a `Merge feat/<x>: …`
  summary, or a PR. Only when the owner asks.
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
cargo run --release -- --nowplaying       # prints what each now-playing source sees
cargo run --release -- --list-devices     # capture devices (names for --device / the Audio in picker)
cargo run --release -- --probe-audio [name]   # capture ~6 s, print band peaks + BPM; exits nonzero on silence
cargo run --release -- --spout-grab <name> out.png   # receive one Spout frame (Windows)
cargo run --release -- --ndi-monitor [name]
cargo run --release -- --list-midi          # MIDI input ports (pad/key controllers)
```

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
- The dev box has no ffmpeg on PATH. imageio-ffmpeg's binary (Python) works
  via the `ffmpeg_path` setting.

## 5. Architecture map

| Path | What |
|---|---|
| `src/main.rs` | The winit app and CLI flags. `render_loop` runs on **its own thread** (Windows' modal move loop would freeze it otherwise). The event thread runs input and the egui panel. They share `Shared` (a `Mutex<Settings>`, a `Mutex<Status>`, and atomics) and talk over `mpsc::Msg`. |
| `src/audio.rs`, `src/sysaudio.rs` | Capture (CPAL devices; the macOS system-output tap lives in `sysaudio.rs`), FFT, onsets and kicks (level-independent: flux > mean×1.8), tempo PLL, groove, `calm` (breakdown), the four-band vocabulary, and the triggered waveform. |
| `src/director.rs` | Auto-pilot: phrase cuts, drop cuts, intensity, and the beats/breakdown modes. |
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
| `src/timeline.rs`, `src/song.rs`, `src/editor.rs`, `src/ai.rs` | The timeline show editor (F2), song playback, and the AI show builder. |
| `src/panel.rs` | The egui control panel. Tabs: Perform, Dancer & FX, Stream, Timeline, Keys, Settings. App-wide preferences (audio in, latency, director rules, AI provider/key) live on **Settings** (`settings_tab`), not in collapsibles on other pages or in the timeline editor. |
| `src/config.rs` | `Settings` (serde, `#[serde(default)]`), actions and hotkeys, and `data_dir()`. |
| `src/midi.rs` | MIDI input (midir): one port, note-ons become `Action`s. |
| `src/snap.rs` | Headless snapshot and benchmark rendering. |
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
- `f32::signum(+0.0)` is **1.0**, not 0.0. An ease like
  `v += rate*dt*(target - v).signum()` never rests: at `v == target` it
  steps up `rate*dt` then eases back — a two-frame judder (this made the
  branding logo wobble horizontally whenever the DJ name was off). Guard
  the at-rest case (see `overlay.rs::ease_vis`).
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

## 9. Docs

- The user-facing docs are in `README.md`. Keep them current when you add
  features, hotkeys or flags. Media lives in `docs/media/`.
- When a hotkey is added, update `config.rs` (`Action::ALL`, `label`,
  `default_key`) and the README key table.
