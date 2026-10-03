import SwiftUI

/// The live page: big pads for the moves a DJ makes mid-set, then every
/// other action the server advertises. Pad titles and lit state come from
/// `PadLook`.
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
    /// Shown on the Timeline page instead.
    private static let elsewhere: Set<String> = ["TimelinePlay", "TimelineRecord", "LatencyDown", "LatencyUp"]

    var body: some View {
        ScrollView {
            VStack(spacing: 14) {
                grid(Self.main, columns: 4, height: h(96))
                SectionHeader(title: "Director")
                    .accessibilityIdentifier("page.perform")
                grid(Self.modes, columns: 3, height: h(60))
                SectionHeader(title: "Show")
                grid(Self.show, columns: 4, height: h(72))
                SectionHeader(title: "Stream")
                grid(Self.stream, columns: 4, height: h(64))
                let placed = Set(Self.main + Self.modes + Self.show + Self.stream).union(Self.hidden).union(Self.elsewhere)
                let rest = conn.info.actions.map(\.key).filter { !placed.contains($0) }
                if !rest.isEmpty {
                    SectionHeader(title: "More")
                    grid(rest, columns: 4, height: h(60))
                }
            }
            .padding(16)
        }
    }

    private func grid(_ keys: [String], columns: Int, height: CGFloat) -> some View {
        let known = keys.filter { conn.info.action($0) != nil }
        return LazyVGrid(columns: Array(repeating: GridItem(.flexible(), spacing: 10),
                                        count: min(columns, max(known.count, 1))), spacing: 10) {
            ForEach(known, id: \.self) { key in
                ActionPad(key: key, height: height)
            }
        }
    }
}
