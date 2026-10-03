import SwiftUI

/// How a pad for one `Action` looks right now: title, a status line, lit or
/// not. The one place pad state is decided, so a toggle reads the same on
/// every page — a toggle pad must light from the field its action flips
/// (e.g. Logo from brand_logo_on, not the branding master brand_on).
struct PadLook {
    var title: String
    var subtitle: String?
    var on = false
    var tint: Color = Theme.accent

    /// Pad-sized titles for actions whose server label is a hotkey
    /// description ("Mark this beat as the downbeat"). Unknown ids keep the
    /// server's label.
    static let short: [String: String] = [
        "PrevScene": "◀︎ Prev", "NextScene": "Next ▶︎", "Strobe": "Strobe", "Blackout": "Blackout",
        "ModeAuto": "Auto", "ModeManual": "Manual", "ModeStatic": "Static",
        "ToggleDancer": "Dancer", "NextClip": "Next clip", "NextStyle": "Dancer look", "CycleCanon": "Canon",
        "CycleFx": "FX", "MarkDownbeat": "Downbeat", "MarkPhrase": "Phrase start", "ShowNowPlaying": "Now playing",
        "ToggleRandom": "Random order", "SaveClip": "Save clip", "RecordSet": "Record set",
        "ToggleLogo": "Logo", "ToggleName": "DJ name", "ToggleTicker": "Ticker",
        "Fullscreen": "Fullscreen", "TimelinePlay": "Timeline play", "TimelineRecord": "Arm record",
        "LatencyDown": "Latency −", "LatencyUp": "Latency +",
    ]

    /// `dancer::STYLES` by index.
    static let styles = ["shadow", "neon", "strobe", "comic", "wire"]

    static func of(_ a: RemoteAction, _ s: ShowState) -> PadLook {
        var l = PadLook(title: short[a.key] ?? a.label)
        let mode = s.mode.lowercased()
        switch a.key {
        // Momentary pads: no lit state; a status line where it helps.
        case "NextScene": l.subtitle = s.nextSceneName
        case "NextClip": l.subtitle = s.clip?.replacingOccurrences(of: "_", with: " ")
        case "NextStyle": l.subtitle = s.dancerStyle.map { styles.indices.contains($0) ? styles[$0] : "style \($0)" } ?? "auto"
        case "LatencyDown", "LatencyUp": l.subtitle = a.key == "LatencyDown" ? "\(Int(s.latencyMs)) ms" : nil

        // Radio group.
        case "ModeAuto": l.on = mode == "auto"
        case "ModeManual": l.on = mode == "manual"
        case "ModeStatic": l.on = mode == "static"

        // Toggles, each lit from the field it flips.
        case "Strobe": l.on = s.strobe; l.tint = .white; l.subtitle = s.strobe ? "on the hits" : nil
        case "Blackout": l.on = s.blackout; l.tint = Theme.danger
        case "ToggleDancer": l.on = s.dancer; l.tint = Theme.dancer; l.subtitle = s.dancer ? "on" : "off"
        case "ToggleRandom": l.on = s.randomOrder; l.subtitle = s.randomOrder ? "random" : "in order"
        case "CycleFx": l.on = s.fx != "Off"; l.subtitle = s.fx == "Off" ? "off" : s.fx
        case "RecordSet": l.on = s.recOn; l.tint = Theme.danger; l.subtitle = s.recOn ? "recording" : nil
        case "ToggleTicker": l.on = s.tickerOn
        case "Fullscreen": l.on = s.fullscreen
        case "ToggleLogo":
            l.on = s.brandOn && s.brandLogoOn == true && s.hasLogo != false
            if s.hasLogo == false { l.subtitle = "none set" }
        case "ToggleName":
            l.on = s.brandOn && s.brandNameOn == true && s.hasName != false
            if s.hasName == false { l.subtitle = "none set" }
        case "TimelinePlay":
            l.on = s.song?.playing == true
            l.subtitle = s.song == nil ? "no song" : (s.song?.playing == true ? "playing" : "paused")
        case "TimelineRecord":
            l.on = s.timelineRecording == true; l.tint = Theme.danger
            if s.timelineRecording == true { l.subtitle = "armed" }
        // Tristate: lit unless auto, current value underneath.
        case "CycleCanon":
            if let c = s.canon { l.subtitle = c.lowercased(); l.on = c.lowercased() == "on" }
        default: break
        }
        return l
    }
}

/// A pad for one advertised action, looked up by id — nothing is drawn if
/// this Trippin doesn't list it.
struct ActionPad: View {
    @EnvironmentObject var conn: Connection
    let key: String
    var height: CGFloat = 64

    var body: some View {
        if let a = conn.info.action(key) {
            let l = PadLook.of(a, conn.state)
            Pad(title: l.title, subtitle: l.subtitle, on: l.on, tint: l.tint, height: height) {
                if key == "Blackout" { Haptics.strong() }
                conn.act(a)
            }
            .accessibilityIdentifier("pad.\(key)")
        }
    }
}
