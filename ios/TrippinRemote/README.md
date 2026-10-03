# Trippin Remote (iOS / iPadOS)

A native SwiftUI remote for Trippin over the LAN: iPhone and iPad, iOS 17+.
It speaks the WebSocket protocol documented at the top of `src/remote.rs`
(port 9138, Bonjour `_trippin._tcp`, PIN-gated).

## Build

The Xcode project is generated from `project.yml` and isn't checked in:

```
brew install xcodegen
cd ios/TrippinRemote
xcodegen
open TrippinRemote.xcodeproj
```

Command-line simulator build:

```
xcodebuild -project TrippinRemote.xcodeproj -scheme TrippinRemote \
  -destination 'platform=iOS Simulator,name=iPhone 17' -derivedDataPath build build
```

Signing uses team `9A6C9KAPRJ` (automatic) for on-device runs.

## Test against a running Trippin

Run Trippin from a scratch folder (never the real settings folder) with a
`trippin.json` like `{"remote_on": true, "remote_pin": "1234"}`, then pick
"Trippin on <host>" in the app, or type the address the Settings → Remote
card shows. `http://<ip>:9138` in a browser serves the same protocol's test
harness, which is handy for checking the server is up.

## Layout

| Path | What |
|---|---|
| `Sources/Remote/Connection.swift` | WebSocket link: hello/PIN, state frames, commands, reconnect with backoff, a 3 s silence watchdog, and the thumbnail cache (requests are dripped out 40 ms apart). |
| `Sources/Remote/Discovery.swift` | `NWBrowser` for `_trippin._tcp`, plus resolving a service to IPv4 host:port on each connect attempt. |
| `Sources/Remote/Models.swift`, `JSON.swift` | The `hello` and `state` frames, read field by field so new server fields never break the app. Action ids are echoed back verbatim. |
| `Sources/Remote/Keychain.swift` | PINs per server (the Bonjour name or host:port), this device only. A rejected PIN is forgotten. |
| `Sources/UI/` | Connect, status bar, Perform pads, the scene grid (tap cuts, hold queues), Dancer, Look/FX and Timeline. iPhone uses tabs; iPad puts the scene grid alongside the controls. |

## TestFlight

App Store Connect app: "Trippin Remote", bundle ID `com.jimmyeao.trippin.remote`,
team `9A6C9KAPRJ`. No capabilities are needed (Bonjour and local network are
Info.plist keys only). `ITSAppUsesNonExemptEncryption` is false: the app uses
plain `ws://` only, so uploads skip the export-compliance question.

To upload a build, bump `CURRENT_PROJECT_VERSION` in `project.yml` first.
App Store Connect rejects a build number it has already seen. Bump
`MARKETING_VERSION` for user-visible releases. Then:

```
xcodegen
xcodebuild -project TrippinRemote.xcodeproj -scheme TrippinRemote -configuration Release \
  -destination 'generic/platform=iOS' -archivePath build/TrippinRemote.xcarchive \
  -allowProvisioningUpdates archive
xcodebuild -exportArchive -archivePath build/TrippinRemote.xcarchive \
  -exportOptionsPlist ExportOptions.plist -exportPath build/export -allowProvisioningUpdates
```

The export signs with Apple Distribution and uploads (`destination: upload`
in `ExportOptions.plist`). It needs an Xcode account on team 9A6C9KAPRJ.
Builds appear under TestFlight after Apple finishes processing them.
Internal testers can install straight away; external testers need Beta App
Review, which can use the built-in demo mode.

Uploaded so far: 0.1.0 (1) and 0.1.0 (2), both on 2026-10-03.
