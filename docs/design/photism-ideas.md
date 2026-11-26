# Ideas from Photism: plan

Photism (photism.app) is a music visualiser for producers. Most of what it
does Trippin already has; these are the ideas worth taking, planned for our
audience (streamers, home and mobile DJs). Build order is the order below.

| # | Feature | Size | Status |
|---|---|---|---|
| 1 | Palette from album art (and art on the now-playing card) | Medium | Built (0a406c7e); LAN agent covers to do |
| 2 | Photosensitivity safe mode | Small | Planned |
| 3 | Lens effects | Medium | Planned |
| 4 | Glide between Looks | Small | Planned |
| 5 | Logo / photo scene | Medium | Planned |
| 6 | Webcam as a source | Large | Planned |
| - | MIDI knobs and faders | Medium | **Might add** (future) |

---

## 1. Palette from album art

**What the user sees:** a new palette chip, `art`, next to `auto`. While a
track with cover art is playing, the scenes take their colours from the
cover; with no art it behaves like `auto`. The now-playing card can show
the cover too (a toggle on the Stream tab).

### Getting the art, source by source

Every source already knows the track; most can also give us the file or an
image. In order of preference per source:

| Source | Where the art comes from | Notes |
|---|---|---|
| Windows media session (Spotify, Apple Music, Tidal, browsers) | `GlobalSystemMediaTransportControlsSessionMediaProperties::Thumbnail()` → `IRandomAccessStreamReference` → bytes | Already in `nowplaying.rs` (the `windows` crate). Spotify desktop publishes a 300 px cover. Needs the `Storage_Streams` feature. |
| macOS Spotify | AppleScript `artwork url of current track` → an `i.scdn.co` JPEG URL | One HTTPS GET (OS trust store, `config::tls()`). |
| macOS Music | AppleScript `raw data of artwork 1 of current track`, written to a temp file | The script writes it; we read the file. |
| Serato 4+ (`master.sqlite`) | The track's file path from the history row, then the art embedded in the file | Check which column holds the path (`file_name`/location in `history_entry`, or a join to the track table). Serato keeps no separate art cache: it reads the file's tags. |
| Serato 3 and earlier (session files) | Field 2 of the `adat` chunk is the file path | We parse it already (`nowplaying.rs` ~435) and throw it away. |
| rekordbox (`master.db`) | `djmdContent.ImagePath` (e.g. `/PIONEER/Artwork/00e/…jpg`, relative to `%APPDATA%/Pioneer/rekordbox/share`), else `FolderPath` and the embedded art | rekordbox caches its own thumbnails, so this works even for files without tags. The owner's machine has the Artwork folder. |
| Mixxx (`mixxxdb.sqlite`) | `library.coverart_source`/`coverart_location` (1 = embedded in the file at `track_locations.location`, 2 = an image file next to it) | All 901 tracks in the owner's library have a cover. |
| VirtualDJ | `tracklist.txt` has no paths; the day's `History/*.m3u` does. Take the newest entry's path, then the embedded art | Match it to the tracklist line by title. |
| Text file | None | Falls back to the online lookup if that's on. |
| Remote (trippin-agent) | The agent runs the same code on the DJ machine and sends the art | Add an optional `art` field (base64 JPEG, at most 256 px, sent once per track) to `now_playing`. The rig can't read the DJ machine's files. |

**Embedded art:** read it with the `lofty` crate (pure Rust: ID3v2 APIC,
FLAC PICTURE, MP4 `covr`, Ogg). Prefer the front-cover picture type. If
the file has none, try `cover.jpg`, `folder.jpg` or `front.jpg` in its
folder.

**Online fallback (opt-in, off by default):** the iTunes Search API (no
key; `itunes.apple.com/search?term=<artist title>&entity=song&limit=1`,
`artworkUrl100` resized to 600). It sends the track name to Apple, so it
is a Settings toggle that says so. Cache results by `artist - title` in
`<data dir>/artcache`.

### Code

- `Track` gains `art: Option<ArtRef>` (a path, URL or bytes). The art is
  read on the now-playing worker thread, never the render thread. Decode
  and downscale it to 64x64 there (`image` is already a dependency).
- `palettes::from_image(&RgbImage) -> [u8; LUT_SIZE*4]`: k-means (k = 5)
  in Lab on the 64x64 pixels, drop near-greys and near-black clusters
  unless nothing else is left, order the clusters dark to light, boost
  saturation a little (scenes square the palette), and build the gradient
  the way the named palettes are built.
- A new track's palette must not cut. The LUT swap is a hard colour cut
  today (`palettes::Auto` uses hysteresis for this reason), so blend the
  old LUT into the new one over a bar on the render thread.
- `"art"` joins `"auto"` as a pseudo-palette: add it to
  `palettes::all_names()`/`is_valid()` (AGENTS §7).
- Now-playing card: draw the 64x64 or a 128 px copy in `overlay.rs` to the
  left of the text.

### Checks

- Unit tests: `from_image` on solid, two-colour and greyscale images; the
  lofty read on a tagged MP3, FLAC and M4A fixture; Mixxx and rekordbox
  queries on fixture databases, like the existing Serato test.
- `--nowplaying` prints the art source and size for each source.
- Live: Spotify on Windows, rekordbox and Mixxx on the owner's machine,
  Serato and Music on the M2.

---

## 2. Photosensitivity safe mode

**What the user sees:** one switch, "Photosensitive-safe", on Settings and
the Stream tab. When it's on, nothing on screen flashes more than three
times a second, and no flash covers the whole frame at full brightness.
This is the WCAG 2.3.1 / Ofcom limit that streaming platforms point to.

**Code:**

- Strobe cues and drum-fill strobes become stutters or nothing; the
  strobe gate in the render loop checks the flag. Fills fall back to the
  stutter path that flight scenes use.
- `present.wgsl` gets a luminance limiter: compare this frame's average
  luminance with an eased average, and clamp a jump to a set step, so a
  scene's own flashes can't exceed it either. That needs one tiny
  downsampled luminance read (the bloom chain's smallest mip already
  exists).
- Text `strobe` fx, `flash` fills and lights white flashes (already capped
  at 3 Hz) obey the flag.

**Checks:** a frame-difference measure over `--snap-at` sequences of every
scene with the flag on: count frames where the luminance change exceeds the
limit. That runs as a test-like CLI pass (`--snap all --snap-flash`).

---

## 3. Lens effects

**What the user sees:** more choices on the Effect picker beside Mirror and
Kaleido: Glass ribs, Glass fan, Fly eye, Anamorphic, Melt, Pixels, Tiles,
Twist. Each one moves with the music like the scenes do (ribs sway with
the flow sine, the melt drips on the energy clock, the pixel size breathes
with the bass presence), never pulsing on the beat.

**Code:**

- Append variants to `Fx` in `config.rs` **before** `Off` (it carries
  `#[serde(other)]`), and handle each in `present.wgsl`'s UV remap. All of
  them are UV warps of the scene texture plus at most a few taps: cheap.
  Glass ribs and fan also add a soft refraction highlight per rib.
- Anamorphic is a horizontal streak of the bloom mip, not a UV warp.
- Update the remote's Fx list, the iOS app's labels, the OSC table and the
  README.

**Checks:** `--snap <scene> --snap-fx <name>` for each over three scenes,
a contact sheet, and the bench (target: under 0.2 ms each at 1080p).

---

## 4. Glide between Looks

**What the user sees:** a "Glide" setting for Looks (Cut, 1 beat, 1 bar,
2 bars). Recalling a Look eases the palette, effect amounts, accent and
master sliders over that time, and changes the scene on the downbeat
at its end, instead of cutting everything at once.

**Code:**

- `looks::apply` stays pure and still writes the target settings. The
  render loop keeps the previous values and eases the continuous ones
  (LUT blend from #1, accent amount, echo, master) toward them over the
  glide, timed in beats from the live tempo.
- There's no scene crossfade today: scene changes are cuts. The glide
  cuts the scene on the downbeat that ends it. A real crossfade would
  render both scenes for its length (twice the cost then), so it's an
  option to try after, not part of this.
- Switches that can't ease (dancer clip, effect type) change at the
  glide's midpoint.

**Checks:** a unit test of the easing schedule; live, recall two Looks
back and forth at each glide setting.

---

## 5. Logo / photo scene

**What the user sees:** a scene, "Logo trip", that runs the branding logo
(or any picked image) through a rotating set of treatments: droste spiral,
melt, halftone, VHS, kaleidoscope tile. The image's own shape changes with
the music: the droste spiral turns on the energy clock, the melt depth
follows the bass presence, halftone dots grow with the mids. The treatment
changes every 8 bars and morphs over two beats, like the laser formations.

**Code:**

- The branding logo is already loaded for the overlay. Upload it once
  into a scene-readable texture: a new binding, or reuse `ext_tex`
  (binding 8) when no Unity show is on air. Changing `common.wgsl`'s
  bindings means bumping `packs::API`.
- `shaders/scenes/logo_trip.wgsl`: each treatment is a UV function, so
  the morph is a mix of two UV results.
- Settings: an image picker for the scene, defaulting to the branding
  logo. With no image it shows the Trippin logo.
- Tags (`abstract`, `bright`, plus a new `branding` tag only if Styles
  need one), a hand-set energy, and the `--snap` check.

---

## 6. Webcam as a source

**What the user sees:** a camera picker on Settings. Scenes that use the
camera appear when a camera is picked: the DJ's own image as an outline
traced in the scene's light, as a kaleidoscope, and as a slit-scan smear.
The dancer layer can take the camera's silhouette instead of a clip.

**Code:**

- Capture on its own thread with `nokhwa` (Media Foundation on Windows,
  AVFoundation on macOS), at 720p30, into the video texture path that
  `video.rs` already uses (`vid_tex`, binding 9). The scene samples it
  with `vid()`.
- Silhouette: a person-segmentation model (MediaPipe selfie segmentation
  as ONNX, run on `rten` as Beat This! is) at 256x256 on a worker thread.
  This is the largest single piece; ship the plain camera scenes first.
- macOS needs the camera permission string in `Info.plist`, and can't be
  compiled from Windows, so the M2 must test it (AGENTS §1).
- Privacy: the camera is off until a camera is picked, and a small
  indicator shows on the panel while it's live.

---

## Might add: MIDI knobs and faders

Photism maps MIDI CC to its sliders and program change to presets. Trippin
takes note-ons only (`midi.rs`), so a DJ controller's knobs can't drive the
accent, echo or master sliders. If we add it:

- `Settings::midi_ccs` (slider → channel + CC), learned on the Keys page
  as notes are.
- Sliders follow the CC value eased over ~50 ms, so a coarse 7-bit knob
  doesn't step.
- Program change n recalls Look n.
