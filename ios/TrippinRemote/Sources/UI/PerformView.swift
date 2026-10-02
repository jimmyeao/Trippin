import SwiftUI

/// The live page: big pads for the moves a DJ makes mid-set, then every
/// other action the server advertises.
struct PerformView: View {
    @EnvironmentObject var conn: Connection
    @Environment(\.verticalSizeClass) private var vsc
    /// Shorter pads when the phone is on its side.
    private func h(_ base: CGFloat) -> CGFloat { vsc == .compact ? (base * 0.7).rounded() : base }

    /// Pads laid out up top, by `Action` id. Ids the server doesn't list
    /// (an older Trippin) are skipped.
    private static let main = ["PrevScene", "NextScene", "Strobe", "Blackout"]
    private static let modes = ["ModeAuto", "ModeManual", "ModeStatic"]
    private static let show = ["ToggleDancer", "NextClip", "CycleFx", "MarkDownbeat",
                               "MarkPhrase", "ShowNowPlaying", "ToggleRandom", "NextStyle"]
    private static let stream = ["SaveClip", "RecordSet", "ToggleLogo", "ToggleName", "ToggleTicker"]
    /// Desktop-window chores that mean nothing from a phone.
    private static let hidden: Set<String> = ["TogglePanel", "ToggleEditor", "LeaveFullscreen", "ReloadShaders"]
    /// Pad-sized titles for actions whose server label is a hotkey
    /// description ("Mark this beat as the downbeat"). Unknown ids keep the
    /// server's label.
    static let short: [String: String] = [
        "NextClip": "Next clip", "NextStyle": "Dancer look", "CycleCanon": "Canon",
        "MarkDownbeat": "Downbeat", "MarkPhrase": "Phrase start", "ShowNowPlaying": "Now playing",
        "ToggleRandom": "Random order", "SaveClip": "Save clip", "RecordSet": "Record set",
        "ToggleLogo": "Logo", "ToggleName": "DJ name", "ToggleTicker": "Ticker",
        "Fullscreen": "Fullscreen", "TimelinePlay": "Timeline play", "TimelineRecord": "Arm record",
    ]

    /// Shown on the Timeline page instead.
    private static let elsewhere: Set<String> = ["TimelinePlay", "TimelineRecord", "LatencyDown", "LatencyUp"]

    var body: some View {
        let s = conn.state
        ScrollView {
            VStack(spacing: 14) {
                grid(Self.main, columns: 4, height: h(96)) { key in
                    switch key {
                    case "PrevScene": return ("◀︎ Prev", nil, false, Theme.accent)
                    case "NextScene": return ("Next ▶︎", s.nextSceneName, false, Theme.accent)
                    case "Strobe": return ("Strobe", s.strobe ? "on the hits" : nil, s.strobe, Color.white)
                    case "Blackout": return ("Blackout", nil, s.blackout, Theme.danger)
                    default: return nil
                    }
                }
                SectionHeader(title: "Director")
                grid(Self.modes, columns: 3, height: h(60)) { key in
                    let m = s.mode.lowercased()
                    switch key {
                    case "ModeAuto": return ("Auto", nil, m == "auto", Theme.accent)
                    case "ModeManual": return ("Manual", nil, m == "manual", Theme.accent)
                    case "ModeStatic": return ("Static", nil, m == "static", Theme.accent)
                    default: return nil
                    }
                }
                SectionHeader(title: "Show")
                grid(Self.show, columns: 4, height: h(72)) { key in
                    switch key {
                    case "ToggleDancer": return ("Dancer", s.dancer ? "on" : "off", s.dancer, Theme.dancer)
                    case "CycleFx": return ("FX", s.fx == "Off" ? "off" : s.fx, s.fx != "Off", Theme.accent)
                    default: return nil
                    }
                }
                SectionHeader(title: "Stream")
                grid(Self.stream, columns: 4, height: h(64)) { key in
                    switch key {
                    case "RecordSet": return ("Record set", s.recOn ? "recording" : nil, s.recOn, Theme.danger)
                    case "ToggleLogo": return ("Logo", nil, s.brandOn, Theme.accent)
                    case "ToggleTicker": return ("Ticker", nil, s.tickerOn, Theme.accent)
                    default: return nil
                    }
                }
                let placed = Set(Self.main + Self.modes + Self.show + Self.stream).union(Self.hidden).union(Self.elsewhere)
                let rest = conn.info.actions.filter { !placed.contains($0.key) }
                if !rest.isEmpty {
                    SectionHeader(title: "More")
                    LazyVGrid(columns: Array(repeating: GridItem(.flexible(), spacing: 10), count: 4), spacing: 10) {
                        ForEach(rest) { a in
                            Pad(title: Self.short[a.key] ?? a.label, height: h(60)) { conn.act(a) }
                        }
                    }
                }
            }
            .padding(16)
        }
    }

    /// A row of pads for the ids the server knows; `look` overrides the
    /// title/subtitle/lit state per id (nil = server label, unlit).
    private func grid(_ keys: [String], columns: Int, height: CGFloat,
                      look: @escaping (String) -> (String, String?, Bool, Color)?) -> some View {
        let actions = keys.compactMap { conn.info.action($0) }
        return LazyVGrid(columns: Array(repeating: GridItem(.flexible(), spacing: 10), count: min(columns, max(actions.count, 1))), spacing: 10) {
            ForEach(actions) { a in
                let l = look(a.key)
                Pad(title: l?.0 ?? Self.short[a.key] ?? a.label, subtitle: l?.1, on: l?.2 ?? false, tint: l?.3 ?? Theme.accent, height: height) {
                    if a.key == "Blackout" { Haptics.strong() }
                    conn.act(a)
                }
                .accessibilityIdentifier("pad.\(a.key)")
            }
        }
    }
}
