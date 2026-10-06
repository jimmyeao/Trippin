# Design: Looks, Styles and Visualisation Packs

Status: **draft for the owner's review. Nothing here is built.** Written 2026-10-05.

Three features, built in this order because each rests on the one before:

1. **Looks**: save the current visual state as one named thing and recall it with a keypress, a pad or the remote.
2. **Styles**: a curated "vibe" (Pop, Rock, Dance...) that steers the auto-pilot: which scenes it may pick, which palettes, how fast it cuts.
3. **Packs**: signed, sold add-ons (e.g. "Xmas Visualisations Pack") that bring new scenes, palettes, Looks and Styles, **including Unity scenes**, without an app release. User-created packs are blocked.

## 0. Decisions (owner, 2026-10-05)

| # | Question | Decision |
|---|---|---|
| D1 | Christmas 2026 Unity scenes: locked-in-engine (route A) or runtime pack loader (route B)? | **Route A**, with the route B spike (P4a) run now; switch to B only if it passes by early November. |
| D2 | How hard to block user-created content? | Friction, not DRM: signed packs plus a signed core manifest (§6). |
| D3 | Naming | **Look** and **Style** in the UI; the dancer picker is relabelled **Dancer look**; code types `SavedLook` and `Theme`. |
| D4 | Licence model | No licence server in v1: signed licence files verified offline. Signing-key design in §5.5 (the owner asked about a long-lived self-signed certificate; see there). |
| D5 | Mac App Store? | **No.** Add-ons are sold outside the Store, so no in-app purchase rules apply. Revisit only if that changes. |
| D6 | Unity licence terms | Checked 2026-10-05, **from secondary sources only** (unity.com is blocked from this environment): see §10. Needs the owner to read the primary terms before the first sale. |

## 1. Today (what the design builds on)

- **WGSL scenes** are loose `shaders/scenes/*.wgsl` files read from disk and hot-reloaded (`render.rs` scene scan). They're not baked into the binary.
- **Unity scenes** are compiled into one player build, downloaded from the `unity-engine-vN` GitHub release and SHA-pinned in `engine.rs::ASSET`. Each has a `unity_<name>.wgsl` stub so the Rust side can list it; the `unity_` prefix is how it is gated on live frames (`main.rs`).
- **Scene state in Settings:** `disabled_scenes`, `favourite_scenes`, `palette` (or `"auto"`), `fx`, `fx_amt`, `fx_auto`, `dancer_enabled`, `dancer_style`, `dancer_size`, `mode`, `random_order`, `phrase_bars`, `cut_on_drops`, `heavy_scenes`, `seasonal`.
- **The director** takes a `usable: &[usize]` list of scene indices every frame and picks from it (`director.rs`); `repick_for_mood` refits the queued next scene to the music's energy.
- **Seasonal scenes already exist** (`config.rs` `SEASONS`: halloween, christmas, fireworks, with date windows hard-coded in the binary). `christmas.wgsl` is a **free built-in scene**. A paid Xmas pack must clearly add to it, not replace or paywall it.
- **Timeline cues** and **remote/OSC commands** key on scene ids. Show state must never leak into `trippin.json` (AGENTS.md §7).
- **Now-playing `Track`** is `{artist, title, source}`. It carries **no genre**. (I earlier guessed Serato/rekordbox genre might already be read. It isn't.)

## 2. Looks

### What it is
A Look is a snapshot of the *visual* state, recalled in one action. It references things that already exist; it contains no code, so user-created Looks are fine and are not "user content" in the §6 sense.

```json
{
  "version": 1,
  "id": "club-red",                  // lowercase slug, unique per owner
  "name": "Club red",
  "scene": "laser_show",             // scene id, or null = leave the auto-pilot alone
  "palette": "inferno",              // palette name, "auto", or null = unchanged
  "fx": { "mode": "kaleido", "amt": 0.4, "auto": false },
  "dancer": { "enabled": true, "style": 3, "clip": "stock_disco", "size": 0.9 },
  "requires": ["xmas"]               // pack ids; informational, see "Unlicensed scenes"
}
```
Fields left out are left unchanged (a Look can be "palette only"). Looks don't hold branding, now-playing, audio or output settings; those are not "looks".

### Behaviour
- **Apply** runs through the same `apply()` path as hotkeys and remote commands, so it records into an armed timeline as ordinary cues and never persists as live state (AGENTS.md §7, the cue-playback rule).
- **Storage:** one JSON file per Look in `<data dir>/looks/`, plus a hotkey/pad slot number. Separate files make export/import trivial (`*.look.json`). Pack-supplied Looks are read-only and live in the pack.
- **Triggers:** new `Action::Look(n)` for n = 1..=16 (hotkeys, MIDI notes via the existing `midi_notes` map, OSC `/trippin/look/<n>`, and a remote command `look`). Per AGENTS.md §9, adding actions means updating `Action::ALL`, `label`, `default_key` and the README key table. 16 default keys is too many; leave them unbound by default.
- **UI:** a "Looks" strip on the Perform tab (tiles with thumbnails; right-click: rename, update from current state, delete, export). "Save current as Look" is one button. Thumbnails reuse the scene thumbnail cache.
- **Unlicensed scenes:** if a Look names a scene from a pack the user doesn't own, apply the rest of the Look, skip the scene, and show "needs Xmas Pack". This is the natural upsell surface and must never error or crash.
- **Dangling ids:** a Look naming an uninstalled scene is kept, not pruned.

### Out of scope for v1
Per-Look text/ticker overlays, scene parameters (scenes have none today), and Look *sequences* (that is the timeline editor's job).

## 3. Styles

### What it is
A Style is a curated *policy* for the auto-pilot, not a snapshot. Picking "Rock" makes Auto choose different scenes, colours and pacing without the DJ touching anything.

```json
{
  "version": 1,
  "id": "rock",
  "name": "Rock",
  "scenes": { "include_tags": ["organic", "stage", "warm"], "exclude_tags": ["neon-grid"],
              "ids": [], "energy": [0.35, 1.0] },
  "calm_scenes": { "include_tags": ["slow", "organic"], "energy": [0.0, 0.45] },
  "palettes": ["ember", "gold", "cobalt"],      // subset; "auto" mood-picker restricted to these
  "director": { "phrase_bars": 8, "cut_on_drops": true, "fx_auto": false, "fx_amt": 0.2 },
  "dancer": { "styles": [0, 2], "enabled": true },
  "default_look": "rock-opening"                // optional Look applied when the Style is chosen
}
```

### Behaviour
- **A Style narrows `usable`.** The director already takes the list of usable scenes; the Style filters it before the call. `repick_for_mood` then refits the queue inside the narrowed set. That's why this is a small change in `main.rs`, not a director rewrite.
- **Precedence (highest first):** the user's `disabled_scenes` and the `heavy_scenes`/GPU gating, then Manual mode (a Style never overrides a manual choice), then the Style's pool, then the global defaults. A Style can never enable something the user disabled.
- **Empty or tiny pool:** if the filter leaves fewer than 4 scenes, widen it to the nearest by tag overlap and show a notice; never leave the director with nothing.
- **Overlay, not persistence:** the Style's director/FX/palette overrides are a runtime overlay (like a timeline borrowing the rig). Only the *selected style id* is saved in `trippin.json`.
- **No audio genre detection in v1.** BPM, groove and band energy cannot reliably tell house from pop; a wrong guess shows a rock crowd dance visuals. The DJ chooses the Style. Optional later: suggest a Style from a genre field, which needs the now-playing readers (Serato, rekordbox) to start extracting genre; treat as a separate small feature.

### Prerequisite: scene tags
Scenes have only an energy rank (`scene_energy.json`) today. Styles need tags (`organic`, `geometric`, `neon`, `stage`, `warm`, `dark`, `slow`, `flight`...). Create `shaders/scene_tags.json` for the ~130 existing scenes (a one-off curation job; also improves the AI show builder) and let packs ship their own. Regenerate/verify it when scenes are added (same rule as `scene_energy.json`, AGENTS.md §7).

### Built-in Styles (v1, curated by hand)
Dance / EDM, House & Techno (dark), Pop, Rock, Hip-hop & R&B, Chill / Lounge, Party / Wedding (mobile DJ). Each ships a pool, 3-5 palettes, director values and an optional default Look. Treat the pools as editorial content that needs the owner's taste; I can draft, the owner decides.

## 4. Pack format

A pack is one `.trippinpack` file (a zip with a fixed layout):

```
manifest.json            # signed
manifest.sig             # ECDSA P-256 over the canonical manifest
payload/…                # files listed (with SHA-256) in the manifest
```

```json
{
  "format": 1,
  "id": "xmas",                       // lowercase slug; also the id namespace
  "name": "Xmas Visualisations Pack",
  "version": "1.0.0",
  "min_app": "0.17.0",
  "kit_api": 1,                       // Unity content only, §5.2
  "platforms": ["windows-x64", "macos-arm64"],
  "season": [["12-01", "12-27"]],     // replaces the hard-coded SEASONS table for pack scenes
  "scenes": [
    { "id": "xmas/snowfall", "kind": "wgsl", "title": "Snowfall", "file": "scenes/snowfall.wgsl",
      "energy": 0.35, "tags": ["slow", "organic"], "heavy": false, "flight": false, "thumb": "thumbs/snowfall.png" },
    { "id": "xmas/workshop", "kind": "unity", "title": "Santa's Workshop", "show": "XmasWorkshop",
      "energy": 0.7, "tags": ["stage"], "heavy": true, "thumb": "thumbs/workshop.png" }
  ],
  "palettes": [ { "id": "xmas/candy", "stops": ["#ff2d2d", "#ffffff", "#0b8a3e"] } ],
  "looks":  [ "looks/xmas-opening.look.json" ],
  "styles": [ "styles/xmas-party.style.json" ],
  "files": { "scenes/snowfall.wgsl": "<sha256>", "...": "..." }
}
```

- **Id namespace:** every pack id is `<pack>/<name>`. Settings, timelines, remote commands and `disabled_scenes` key on ids (AGENTS.md §6), so a pack can never shadow or collide with a core scene. The `unity_` prefix test is replaced for pack scenes by `kind`.
- **Install location:** `<data dir>/packs/<id>/<version>/`. Core files are never modified by a pack.
- **Missing pack:** ids from a removed or unlicensed pack stay in settings, timelines and Looks (never pruned) and are skipped with a notice, so reinstalling restores everything.
- **Seasonal generalisation:** pack `season` windows feed the same `in_season` logic. The hard-coded `SEASONS` table stays for built-ins.

## 5. Packs: loading, Unity content, licensing

### 5.1 WGSL, palettes, Looks, Styles (the easy half)
Load by adding the pack's `payload/scenes` to the scene scan after verification (§6). Each scene is naga-validated at load and again at pack build. Palettes, Looks and Styles are plain data merged into the pickers, read-only, tagged with the pack name.

### 5.2 Unity content: the real problem
The engine is a single compiled player; its shows are C# classes registered at build time (`StageBuilder`, `ShowManager.shows[]`). **Unity cannot run arbitrary new C# in an IL2CPP build, but the desktop player appears to use the Mono backend** (`ProjectSettings.asset` lists only `Android` as IL2CPP; desktop falls back to Mono). That matters: Mono can load a managed assembly at runtime. **This must be confirmed on a real build**; it is the keystone of route B.

Three ways to ship a Unity scene with a pack:

| Route | How | Good | Bad |
|---|---|---|---|
| **A. Locked in the engine** | The pack's shows are compiled into the normal engine release, but hidden and refused unless Trippin passes a valid entitlement. | Works today, no new technology. Same release path as v14/v15. | Every pack means a new engine release and a new ~36 MB download for *everyone*, owners or not. The shows sit, unlocked in code, in a public zip. |
| **B. Runtime-loaded content** | Pack contains Unity AssetBundles (prefabs, meshes, materials, shaders) plus a signed managed DLL with the show logic. The engine's new `PackLoader` verifies and loads them and registers the shows with `ShowManager`. | Packs ship independently of engine releases. Only owners download them. Scales to many packs. | New machinery: see below. |
| **C. One player per pack** | Each pack ships its own full Unity player. | Isolation. | Process start of ~1-2 s on every switch into the pack (no drop-synced cuts), duplicated GPU memory, larger downloads. Not recommended. |

**What route B needs**
1. **A stable SDK:** extract `Kit.cs`, `Rx`, `Eased`, `DropDirector`, the pools and `RobotKit` into an assembly (`Trippin.Kit`, asmdef) with a `kit_api` major version. Today these are internal to the project; the moment packs compile against them they are a public API and breaking changes cost money. The manifest's `kit_api` is checked against the engine's.
2. **A pack build tool** (editor script + `tools/pack_build.py`) that builds AssetBundles *per platform with the engine's exact Unity version and URP settings* (bundles are not portable across Unity versions), compiles the show assembly, hashes everything, and signs the manifest.
3. **Dynamic show registration:** `ShowManager.shows/names` are build-time arrays; they become a list that `PackLoader` appends to.
4. **Engine version coupling:** when the engine moves to a new Unity editor version, **every pack must be rebuilt**. Keep the editor version pinned, and record it in the manifest so Trippin can say "pack needs rebuilding" rather than failing.
5. **macOS:** the app is hardened/notarised. Loading a managed assembly is data, not `dlopen`, so I expect it to work (the Mono JIT already needs its entitlement), **but only a build on the M2 proves it.**

**Spike result (m2mac, 2026-10-05, branch `spike/pack-loader-m2`, Unity 6000.3.25f1): route B works on macOS.**

| Check | Result |
|---|---|
| Scripting backend | **Mono** (`Managed/Assembly-CSharp.dll` + `libmonobdwgc-2.0.dylib` on Mac; `MonoBleedingEdge/` on Windows). Runtime assemblies are possible. |
| Build a pack | AssetBundle via `BuildPipeline.BuildAssetBundles` per platform (31 s, ~10 KB); the show DLL compiled **outside** Unity with the editor's Roslyn against the built player's `Managed/` folder (4.6 KB). |
| Load it | `PackLoader` at the top of `ShowManager.Start`: reads `manifest.json`, SHA-256 checks every file, `Assembly.Load`, loads the bundle, creates the show on an *inactive* GameObject (`KitShow.Awake` calls `Build()`), copies the kit's public fields by reflection and appends to `shows/names`. **25 ms** (9-14 ms warm). Needed one new public field (`KitShow.pack`) and one call. |
| Shape/motion on real music | Orbs sized by `rx.Spec`/`rx.kick`, ring on `rx.clk`, bob on `rx.beatS`; median frame change 1.25 on a real feed. |
| Tamper check | One flipped byte in the DLL or the bundle is refused ("hash mismatch"). |
| Shaders from a bundle | `Trippin/Robot` rendered correctly on Metal, no stripping problem. |
| Cross-platform bundles | A Windows-built bundle on macOS fails ("shader compiler platform 14 is not available", magenta). **Bundles are per-platform**, as expected. |
| Hardened runtime | The engine **as shipped is ad-hoc signed and not notarised** (it runs because Trippin downloads it itself, so no quarantine flag); `Trippin.app` is hardened, the engine inside nothing. If the engine is ever hardened, Mono needs `com.apple.security.cs.allow-jit` and `allow-unsigned-executable-memory` (without them it crashes in Mono start-up, with or without packs). `disable-library-validation` is **not** needed. Packs add no requirement of their own. |

**Windows result (median, 2026-10-05, branch `spike/pack-loader-win` ddf70eb4; Windows 11 Enterprise 26H2, D3D11, RTX 3060 Laptop): route B works on Windows too.**

| Check | Result |
|---|---|
| Scripting backend | Mono (`Managed/Assembly-CSharp.dll` + `MonoBleedingEdge/`, no `GameAssembly.dll`). |
| Build and load | Player + Win64 bundle built; DLL compiled with the editor's Roslyn against the built `Managed/`. Loads in **13-15 ms**, renders and moves on the synthetic groove (**not** tested on a real-track feed: that checkout's `trippin.exe` predates `--dump-feed`). |
| Downloaded files | Loads with the Mark-of-the-Web (`ZoneId=3`) on the DLL and bundle. |
| Antivirus | Microsoft Defender and Defender for Endpoint logged nothing and caused no delay. (One machine only; other AV products untested.) |
| Awkward paths | Loads from `Pack Dir Ünïcødé ✨\Xmas Pâck`. |
| Instanced draws | A bundle-only shader with instancing enabled **in the material asset** draws correctly. |
| Tamper check | A flipped byte in the DLL or the bundle is refused ("hash mismatch on <file>"). |

**New finding (median):** a bundle material with instancing *off* does not draw nothing silently; `RenderMeshInstanced` throws "Material needs to enable instancing" **every frame**, which aborts the show's `Frame()` before the camera update. **The loader must wrap a pack show's `Frame()` in try/catch** (log once, then disable that show) so a broken pack can never freeze the camera or take the stage down.

**Corrections to this document:** the engine is not "hardened/notarised" today (§5.2 point 5 assumed it was); and a refused pack's scene name is unknown to the player, which then falls back to the first show, so **Trippin must hide the scenes of any pack it refuses** (§6).

**Still unproven (do before relying on route B):**
1. ~~**Windows.**~~ Done, passes (above). Not yet run on a real-track feed.
2. **Kit ABI across engine releases.** The spike's DLL binds to the exact `Assembly-CSharp` it was compiled against. A DLL built for engine v15 may throw `MissingMethodException` on a v16 whose `Kit` internals changed. The `Trippin.Kit` asmdef plus `kit_api` version is what makes this survivable, and **it is mandatory, not optional**.
3. ~~**Instanced draws from a bundle material**~~ Done on Windows, passes with instancing on the asset (above); not yet checked on Metal.
4. **A quarantined, browser-downloaded pack folder**, and **real notarisation** (needs the owner's Apple credentials).
5. **Signature algorithm in the engine** (§6): Unity's Mono has no built-in Ed25519.

**Christmas recommendation:** build the Xmas scenes so they work either way (kit shows are already data-light and `Kit`-based). Ship route A for Christmas if the spike hasn't passed by early November. Cost of route A: one engine release in which the Xmas shows are present but locked. Nothing is wasted: the shows move into a pack when route B lands.

### 5.3 Performance and quality gate for pack scenes
Every pack scene must pass, before signing: `--check-shaders`, the GPU baseline (`perf.rs`; sub-30 fps on the reference GPU blocks release), and the m2mac real-feed regression (`--dump-feed`/`-replayFeed`) so a pack never ships with the dull-on-real-music problem found in v14. Add these to a `docs/pack-checklist.md` when the format is real.

### 5.4 Packs in the scene list
Pack scenes appear in the library with a pack badge and a lock icon when unlicensed (visible, not hidden: it's the shop window). Locked scenes are never offered to the director or the AI builder. Pack-supplied `energy` and `tags` feed `repick_for_mood`, Styles and the AI builder in place of `scene_energy.json`.

### 5.5 Licensing and delivery
- **Distribution:** the engine release is public on GitHub today. Paid packs must not be. Host them where a signed URL or a licence is required (object storage behind a small function), or publish them encrypted.
- **Signing keys (answering D4's certificate question):** a self-signed certificate is the wrong shape. We don't want certificate *trust*; the app trusts exactly the public keys we embed, so X.509 adds parsing code, an expiry that can brick every issued licence, and nothing else. Use **raw keypairs**, and prefer **ECDSA P-256 over Ed25519**: Trippin verifies with `ring` (already in `Cargo.lock`; confirm with `cargo tree -i ring`) and the Unity engine verifies with Mono's built-in `ECDsa`, whereas Unity's Mono has no built-in Ed25519 (check on the first spike follow-up; if it really is missing, Ed25519 would need a vendored managed implementation inside the engine). A key has no expiry to lapse. "Long validity" is replaced by the thing that actually matters: the **ability to rotate**. Concretely: (1) two separate keys, a *pack key* and a *licence key*, so one leak doesn't forge both; (2) each signed file carries a `kid`, and the app embeds a *list* of trusted public keys (current plus the next, pre-shipped), so rotation is an app update that already trusts the new key before it is used; (3) the private keys live offline or on a hardware token, never in the repo or CI by default (the pack key may sit in CI secrets if the owner accepts that); (4) licences never expire on key age, only on their own optional expiry field; (5) a revocation list can distrust a `kid` or a single licence id. If the owner meant the *Windows code-signing certificate* for the installer, that is a different, unrelated thing.
- **v1 (no server):** the pack's payload is encrypted with a per-pack key; a **licence file** (`xmas.license`), signed by us, holds the pack id, the purchaser's email hash, an optional expiry and the sealed key. Trippin verifies the signature offline, decrypts, installs. The payment platform's delivery email carries the licence file. Sharing a licence file shares the pack; that is accepted friction.
- **v2 (if piracy bites):** online activation against a minimal service (activate / deactivate / revocation list), a machine limit, and an offline grace period of ~30 days. Don't build it before it's needed.
- **Refunds:** a revocation list checked opportunistically; offline users keep working.
- **Price and bundling** are business decisions for the owner; the format does not constrain them (single pack, bundle, seasonal re-release).

## 6. Blocking user-created content (and what that really means)

**Goal:** only content we signed runs. **Reality:** this is friction, not DRM. A user with a debugger or the source can bypass it, and the repo is open to agents and contributors. Design for "honest people can't accidentally or casually load third-party packs and can't redistribute ours trivially".

1. **Pack signature:** ECDSA P-256 over the manifest (see §5.5 for why not Ed25519), with every payload file hashed in the manifest. Trippin embeds the **public** keys (current + next, to allow rotation) and refuses a pack whose signature or any hash fails. The **private key never enters the repo**: it lives in CI secrets or a hardware token; document custody in a private note.
2. **Unity side:** the engine verifies the same signature before `Assembly.Load` or loading an AssetBundle. Trippin does it first; the engine re-checks, so a tampered file dropped into the packs folder is still rejected.
3. **A hole in today's app:** an installed Trippin reads **any** `.wgsl` file placed in its `shaders/scenes/` folder (`render.rs` scene scan). A user can already drop their own scene in. If "block user content" is to mean anything, the core set also needs a **build-time signed manifest** (hash of every shipped scene) and release builds must ignore files not in it. That needs a developer escape hatch (`TRIPPIN_DEV=1` / a cargo feature) because agents and contributors rely on loose files and hot-reload (AGENTS.md §4). The escape hatch is itself a bypass; that is acceptable under "friction".
4. **Looks and Styles stay open.** They hold no code. A user-made Look can only reference scenes the user owns, so it adds no new content. Decide explicitly if user-authored Styles are allowed; I'd allow them (they're pools of owned scenes).
5. **No network code execution.** Packs are data plus shaders plus (route B) a signed assembly. No scripting language, no URLs fetched at runtime by pack content.
6. **Crash containment:** a pack scene that hangs or resets the GPU must not brick the app. Record "pack scene running" in a sentinel file; on next launch after an unclean exit, disable that pack and say why. This matters more once paid content runs in front of a live audience.

## 7. UI

- **Settings → Packs** card: installed packs (name, version, owner status, size), Install from file, Remove, "Check for updates", the licence status, and last verification result in plain words ("signature OK", "needs app 0.17").
- **Perform tab:** the Looks strip (§2); a Style picker (a segmented control or dropdown next to Mode).
- **Library:** pack badge and lock (§5.4); a "Packs" filter chip next to the existing All/2D/3D/Seasonal chips.
- **iOS remote and OSC:** expose `looks` and `styles` lists in `hello` and add `look` / `style` commands. Protocol changes are additive and must leave old clients working (AGENTS.md §8.5).
- **README and `unity/README.md`** updated in the same commits as each feature (AGENTS.md §9).

## 8. Phases

| Phase | Deliverable | Size | Depends on |
|---|---|---|---|
| P0 | Naming decision; `scene_tags.json` for the core scenes | M (curation) | D3 |
| P1 | Looks: save/apply/export, hotkey/MIDI/OSC/remote, tests | M | P0 |
| P2 | Styles: schema, 7 built-ins, director filter, picker, tests | M | P0, P1 |
| P3 | Pack container, signing, verification, WGSL/palette/Look/Style content, Settings → Packs, pack build tool, signed core manifest | L | P1, P2, D2, D4 |
| P4a | **Unity spike** (Mono assembly + AssetBundle on Win/M2). **done and passed on macOS and Windows, 2026-10-05** | S | none |
| P4b | Route B: `Trippin.Kit` SDK split, `PackLoader`, dynamic registration, pack build integration | L | P4a, P3 |
| P5 | Commerce: payment platform, licence issuing, delivery hosting, refunds | M | D4, D5, D6 |
| Xmas | First pack: Xmas scenes via route A or B | M | P3 minimum |

P4a is independent and cheap: start it first, in parallel with P0-P2.

## 9. Tests

- **Unit:** manifest verification (good, tampered file, tampered manifest, wrong key, unknown key, expired licence, wrong platform, `min_app` too new, `kit_api` mismatch); id namespacing and collision; dangling ids preserved across save/load; Style filter (pool honoured, empty pool widened, `disabled_scenes` always wins, Manual unaffected); Look apply (partial Looks leave other fields alone, unlicensed scene skipped, idempotent, records as cues, never persists live state); the licence sealed-key round-trip.
- **Property/robustness:** a corrupt or truncated pack never panics the app; a pack scene that fails `naga` is excluded with a notice.
- **Manual (M2 and Windows):** install, remove and reinstall a pack; locked scenes visible but unusable; GPU-hang sentinel recovery; Look/Style in a live set with real audio via the system tap (m2mac has this path); `-replayFeed` regression for each pack scene.

## 10. Risks and open questions

- **The SDK freeze (route B)** is the biggest hidden cost: from the first sold pack, `Kit`/`Rx` changes must be backward compatible or versioned. We have been rewriting those freely (the beatS fixes this week were a breaking behaviour change). Expect to slow down there.
- **Unity version pinning** couples every pack to the engine's editor version (§5.2).
- **Route A's leak:** locked shows are present, unlocked in code, in a public zip. Acceptable for honest-user friction, not for anything stronger.
- **Support burden:** a signed paid pack that misbehaves is *our* bug, not the user's. The quality gate (§5.3) is the cost of selling.
- **Content licensing:** every asset, audio-reactive trademark and any music in a pack must be original or licensed for commercial redistribution.
- **Demand:** nothing yet shows how many users would pay. A cheap test is to ship the Xmas pack as the *only* pack and measure before building P5 beyond the minimum.
- **Free built-in `christmas` scene** must stay free; the pack's pitch is "more", not "the same, paid".
- **Unity licence (D6).** Findings from secondary sources (unity.com itself was unreachable): Unity Personal is free below **US$200,000 total revenue *and* funding** over the last 12 months, measured for the whole company or legal entity, not per product; above it **Unity Pro** is required (about US$210 per seat per month in the sources found). The Runtime Fee was cancelled in September 2024, and the "Made with Unity" splash is optional on all plans from Unity 6. So selling packs is allowed on Personal while the *total* stays under the threshold, and the Christmas pack's revenue counts toward it. Not verified against the primary terms; the owner should read them before the first sale.
- **Mac App Store (D5):** decided no. If that changes, add-ons would need Apple's in-app purchase.
