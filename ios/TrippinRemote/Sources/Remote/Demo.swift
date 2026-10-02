import Foundation
import UIKit

/// A stand-in Trippin that lives inside the app: it answers the same JSON
/// frames as `src/remote.rs` (hello, ~10 Hz state, thumbs) so the whole UI
/// runs with no computer on the network — for "Try the demo", App Review,
/// and screenshot tests. Commands update its show state the way the real
/// server's would; nothing is sent anywhere.
@MainActor
final class DemoServer {
    private let deliver: (String) -> Void
    private var timer: Timer?
    private var started = Date()

    // Show state.
    private var scene = 0
    private var next: Int?
    private var barStart = Date()
    private var mode = "Auto"
    private var palette = "party"
    private var fx = "Off"
    private var fxAmt = 0.6
    private var fxAuto = true
    private var blackout = false
    private var strobe = false
    private var dancer = true
    private var clip = 0
    private var dancerSize = 0.7
    private var dancerTrails = false
    private var phraseBars = 16
    private var cutOnDrops = true
    private var latencyMs = 40.0
    private var npSize = 1.0
    private var randomOrder = false
    private var fullscreen = true
    private var canon = "Auto"
    private var tlRecording = false
    private var style: Int?
    private var brandOn = true
    private var brandLogoOn = true
    private var brandNameOn = false
    private var brandOpacity = 0.9
    private var tickerOn = false
    private var tickerSpeed = 1.0
    private var tickerText = "Thanks for tuning in"
    private var recOn = false
    private var songPlaying = false
    private var songPos = 0.0
    private var songAt = Date()

    private let bpm = 126.0
    private var beatLen: Double { 60 / bpm }

    static let scenes = [
        "laser_show", "stage_rig", "aurora", "arch_run", "neon_coaster", "rave_hall",
        "crystal_cave", "fractal_flight", "bokeh_lights", "synthwave", "ocean", "nebula",
        "led_wall", "beam_sweep", "fire_mandala", "glass_monoliths", "warehouse_haze",
        "spiral_galaxy", "silk_flow", "tunnel", "metaballs", "city_rain", "light_trails", "vortex",
    ]
    static let heavy: Set<String> = ["arch_run", "crystal_cave", "fractal_flight", "glass_monoliths",
                                     "neon_coaster", "warehouse_haze"]
    static let clips = ["hiphop_01", "house_shuffle", "popping", "vogue", "breakdance_toprock", "stock_amber"]
    static let palettes = ["rainbow", "party", "ocean", "forest", "sunset", "lava", "fire", "ice",
                           "cyber", "magenta", "pastel", "gold"]
    static let tracks = [("Bicep", "Glue"), ("Fred again..", "Delilah"), ("Peggy Gou", "Nanana"),
                         ("Charlotte de Witte", "Overdrive")]
    /// Ids and labels as `config::Action` serialises them.
    static let actions: [(String, String)] = [
        ("NextScene", "Next scene"), ("PrevScene", "Previous scene"),
        ("ModeAuto", "Director: auto"), ("ModeStatic", "Director: static"), ("ModeManual", "Director: manual"),
        ("ToggleRandom", "Random / sequential order"), ("ToggleDancer", "Dancer on / off"),
        ("NextClip", "Next dancer routine"), ("NextStyle", "Next dancer look"),
        ("CycleCanon", "Canon: auto / on / off"), ("Blackout", "Blackout (fade to black)"),
        ("Strobe", "Strobe on / off (flashes on the drum hits)"), ("Fullscreen", "Fullscreen on / off"),
        ("MarkDownbeat", "Mark this beat as the downbeat"), ("LatencyDown", "Latency -5 ms (visuals later)"),
        ("LatencyUp", "Latency +5 ms (visuals earlier)"), ("CycleFx", "Cycle FX"),
        ("TimelinePlay", "Timeline play / pause"), ("TimelineRecord", "Timeline record"),
        ("ShowNowPlaying", "Show the now-playing card"), ("SaveClip", "Save a clip (the last 30 s)"),
        ("RecordSet", "Record set on / off"), ("MarkPhrase", "Mark phrase start"),
        ("ToggleLogo", "Logo on / off"), ("ToggleName", "DJ name on / off"), ("ToggleTicker", "Ticker on / off"),
    ]

    init(deliver: @escaping (String) -> Void) {
        self.deliver = deliver
    }

    func start() {
        started = Date()
        barStart = Date()
        emit([
            "type": "hello", "ok": true, "version": "demo", "addr": "demo",
            "scenes": Self.scenes,
            "heavy": Self.scenes.map { Self.heavy.contains($0) },
            "clips": Self.clips,
            "palettes": Self.palettes,
            "actions": Self.actions.map { ["id": $0.0, "label": $0.1] },
        ])
        timer = Timer.scheduledTimer(withTimeInterval: 0.1, repeats: true) { [weak self] _ in
            Task { @MainActor in self?.tick() }
        }
        tick()
    }

    func stop() {
        timer?.invalidate()
        timer = nil
    }

    private func emit(_ obj: [String: Any]) {
        guard let data = try? JSONSerialization.data(withJSONObject: obj) else { return }
        deliver(String(decoding: data, as: UTF8.self))
    }

    private var barsIn: Int { Int(Date().timeIntervalSince(barStart) / (beatLen * 4)) }

    private func cut(to i: Int) {
        scene = (i + Self.scenes.count) % Self.scenes.count
        next = nil
        barStart = Date()
    }

    private func tick() {
        // The director: in Auto, cut on the phrase boundary (to the queued
        // scene if there is one).
        if mode == "Auto", barsIn >= phraseBars {
            cut(to: next ?? scene + 1)
        }
        let beats = Date().timeIntervalSince(started) / beatLen
        if songPlaying {
            songPos = min(songPos + Date().timeIntervalSince(songAt), 372)
        }
        songAt = Date()
        let track = Self.tracks[Int(Date().timeIntervalSince(started) / 90) % Self.tracks.count]
        emit([
            "type": "state",
            "bpm": bpm, "conf": 0.92, "beat_in_bar": Int(beats) % 4, "fps": 60, "silent": false,
            "scene": scene, "scene_name": Self.scenes[scene],
            "next_scene": next.map { $0 as Any } ?? NSNull(),
            "next_scene_name": next.map { Self.scenes[$0] as Any } ?? NSNull(),
            "bar_in_scene": min(barsIn, phraseBars - 1), "bars_total": mode == "Auto" ? phraseBars : 0,
            "song": ["playing": songPlaying, "pos": songPos, "len": 372.0, "name": "Friday warm-up"],
            "strobe": strobe, "cut_on_drops": cutOnDrops,
            "clip": dancer ? Self.clips[clip] : NSNull(), "blackout": blackout, "fullscreen": fullscreen,
            "fx": fx, "groove": 0.8, "calm": 0.0,
            "np": ["artist": track.0, "title": track.1], "rec_on": recOn,
            "mode": mode, "dancer": dancer, "dancer_style": style.map { $0 as Any } ?? NSNull(), "palette": palette,
            "random_order": randomOrder, "canon": canon, "timeline_recording": tlRecording, "phrase_bars": phraseBars, "fx_amt": fxAmt, "fx_auto": fxAuto,
            "dancer_size": dancerSize, "dancer_trails": dancerTrails, "latency_ms": latencyMs,
            "np_size": npSize, "brand_on": brandOn, "brand_opacity": brandOpacity,
            "brand_logo_on": brandLogoOn, "brand_name_on": brandNameOn, "has_logo": true, "has_name": true,
            "ticker_on": tickerOn, "ticker_speed": tickerSpeed, "ticker_text": tickerText,
        ])
    }

    // MARK: commands

    func receive(_ m: [String: Any]) {
        switch m["cmd"] as? String {
        case "action": act(m["action"] as? String ?? "")
        case "goto_scene": if let i = index(m["scene"]) { cut(to: i) }
        case "queue_next": if let i = index(m["scene"]) { next = i }
        case "show_clip":
            if let c = m["clip"] as? String, let i = Self.clips.firstIndex(of: c) { clip = i; dancer = true }
        case "set": set(m["key"] as? String ?? "", m["value"])
        case "transport":
            switch m["op"] as? String {
            case "toggle", "play", "pause": songPlaying.toggle()
            case "stop": songPlaying = false; songPos = 0
            case "seek": songPos = (m["pos"] as? Double) ?? 0
            default: break
            }
        case "thumb":
            if let s = m["scene"] as? String { thumb(s) }
        default:
            emit(["type": "err", "msg": "bad frame: unknown cmd"])
        }
        tick()
    }

    private func index(_ v: Any?) -> Int? {
        if let i = v as? Int { return Self.scenes.indices.contains(i) ? i : nil }
        if let s = v as? String { return Self.scenes.firstIndex { $0.caseInsensitiveCompare(s) == .orderedSame } }
        return nil
    }

    private func act(_ id: String) {
        switch id {
        case "NextScene": cut(to: scene + 1)
        case "PrevScene": cut(to: scene - 1)
        case "ModeAuto": mode = "Auto"; barStart = Date()
        case "ModeManual": mode = "Manual"
        case "ModeStatic": mode = "Static"
        case "ToggleDancer": dancer.toggle()
        case "ToggleRandom": randomOrder.toggle()
        case "Fullscreen": fullscreen.toggle()
        case "NextStyle": style = ((style ?? 0) + 1) % 3
        case "CycleCanon": canon = ["Auto": "On", "On": "Off"][canon] ?? "Auto"
        case "TimelineRecord": tlRecording.toggle()
        case "NextClip": clip = (clip + 1) % Self.clips.count
        case "Blackout": blackout.toggle()
        case "Strobe": strobe.toggle()
        case "CycleFx": fx = fxModes[((fxModes.firstIndex(of: fx) ?? 0) + 1) % fxModes.count]
        case "LatencyDown": latencyMs = max(0, latencyMs - 5)
        case "LatencyUp": latencyMs = min(200, latencyMs + 5)
        case "TimelinePlay": songPlaying.toggle()
        case "RecordSet": recOn.toggle()
        // As main.rs: showing a piece also enables the block.
        case "ToggleLogo": brandLogoOn.toggle(); brandOn = brandOn || brandLogoOn
        case "ToggleName": brandNameOn.toggle(); brandOn = brandOn || brandNameOn
        case "ToggleTicker": tickerOn.toggle()
        case "MarkDownbeat", "MarkPhrase": barStart = Date()
        default: break
        }
    }

    private func set(_ key: String, _ v: Any?) {
        let d = (v as? Double) ?? (v as? Int).map(Double.init)
        let b = v as? Bool
        switch key {
        case "palette": if let s = v as? String, Self.palettes.contains(s) { palette = s }
        case "fx": if let s = v as? String, fxModes.contains(s) { fx = s }
        case "fx_amt": if let d { fxAmt = min(max(d, 0), 1) }
        case "fx_auto": if let b { fxAuto = b }
        case "dancer_size": if let d { dancerSize = min(max(d, 0.4), 1) }
        case "dancer_trails": if let b { dancerTrails = b }
        case "phrase_bars": if let d { phraseBars = min(max(Int(d), 1), 128) }
        case "cut_on_drops": if let b { cutOnDrops = b }
        case "latency_ms": if let d { latencyMs = min(max(d, 0), 200) }
        case "np_size": if let d { npSize = min(max(d, 0.5), 2) }
        case "brand_opacity": if let d { brandOpacity = min(max(d, 0.1), 1) }
        case "ticker_speed": if let d { tickerSpeed = min(max(d, 0.3), 3) }
        case "ticker_text": if let s = v as? String { tickerText = s }
        default: emit(["type": "err", "msg": "\"\(key)\" isn't remotely settable"])
        }
    }

    /// A stand-in thumbnail: layered glows in the palette of the scene's
    /// name hash, so tiles look like a visuals grid rather than grey boxes.
    private func thumb(_ scene: String) {
        let seed = scene.unicodeScalars.reduce(UInt32(2166136261)) { ($0 ^ $1.value) &* 16777619 }
        let size = CGSize(width: 320, height: 180)
        let img = UIGraphicsImageRenderer(size: size).image { ctx in
            let c = ctx.cgContext
            UIColor(red: 0.04, green: 0.04, blue: 0.06, alpha: 1).setFill()
            c.fill(CGRect(origin: .zero, size: size))
            var r = seed
            func rnd() -> CGFloat { r = r &* 1664525 &+ 1013904223; return CGFloat(r >> 8) / CGFloat(1 << 24) }
            let hue = rnd()
            for i in 0..<7 {
                let h = (hue + CGFloat(i) * 0.09).truncatingRemainder(dividingBy: 1)
                let col = UIColor(hue: h, saturation: 0.85, brightness: 1, alpha: 1)
                let center = CGPoint(x: rnd() * size.width, y: rnd() * size.height)
                let radius = 30 + rnd() * 110
                let colors = [col.withAlphaComponent(0.75).cgColor, col.withAlphaComponent(0).cgColor] as CFArray
                if let g = CGGradient(colorsSpace: CGColorSpaceCreateDeviceRGB(), colors: colors, locations: [0, 1]) {
                    c.drawRadialGradient(g, startCenter: center, startRadius: 0, endCenter: center, endRadius: radius, options: [])
                }
            }
        }
        guard let png = img.pngData() else { return }
        emit(["type": "thumb", "scene": scene, "png_b64": png.base64EncodedString()])
    }
}
