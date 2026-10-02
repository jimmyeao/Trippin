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
