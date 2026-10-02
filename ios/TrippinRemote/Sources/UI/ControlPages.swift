import SwiftUI

/// A slider bound to a `set` key: follows the server's value until the
/// finger is on it, sends at most every 60 ms while dragging and once on
/// release (so the last value always lands).
struct RemoteSlider: View {
    @EnvironmentObject var conn: Connection
    let title: String
    let key: String
    let value: Double
    let range: ClosedRange<Double>
    var step: Double? = nil
    var format: (Double) -> String = { String(format: "%.2f", $0) }
    @State private var local: Double?
    @State private var lastSent = Date.distantPast

    var body: some View {
        let shown = local ?? value
        VStack(alignment: .leading, spacing: 4) {
            HStack {
                Text(title).foregroundStyle(Theme.text)
                Spacer()
                Text(format(shown)).font(.system(.body, design: .monospaced)).foregroundStyle(Theme.muted)
            }
            Slider(value: Binding(get: { shown }, set: { v in
                local = v
                if Date().timeIntervalSince(lastSent) > 0.06 { push(v) }
            }), in: range) { editing in
                if !editing, let v = local {
                    push(v)
                    // Hold the local value briefly so the slider doesn't
                    // snap back before the next state frame echoes it.
                    DispatchQueue.main.asyncAfter(deadline: .now() + 0.4) { local = nil }
                }
            }
        }
    }

    private func push(_ v: Double) {
        lastSent = Date()
        var out = v
        if let step { out = (v / step).rounded() * step }
        conn.set(key, out)
    }
}

/// A toggle bound to a boolean `set` key.
struct RemoteToggle: View {
    @EnvironmentObject var conn: Connection
    let title: String
    let key: String
    let value: Bool
    var body: some View {
        Toggle(title, isOn: Binding(get: { value }, set: { v in
            Haptics.press()
            conn.set(key, v)
        }))
        .tint(Theme.accent)
        .foregroundStyle(Theme.text)
    }
}

// MARK: - Dancer

struct DancerView: View {
    @EnvironmentObject var conn: Connection

    var body: some View {
        let s = conn.state
        ScrollView {
            VStack(spacing: 14) {
                HStack(spacing: 10) {
                    ActionPad(key: "ToggleDancer")
                    ActionPad(key: "NextClip")
                    ActionPad(key: "NextStyle")
                    ActionPad(key: "CycleCanon")
                }
                Card {
                    RemoteSlider(title: "Size", key: "dancer_size", value: s.dancerSize, range: 0.4...1.0)
                    RemoteToggle(title: "Trails", key: "dancer_trails", value: s.dancerTrails)
                }
                SectionHeader(title: "Clips")
                if conn.info.clips.isEmpty {
                    Text("No dancer clips on this Trippin.").foregroundStyle(Theme.muted)
                }
                LazyVGrid(columns: [GridItem(.adaptive(minimum: 140), spacing: 10)], spacing: 10) {
                    ForEach(conn.info.clips, id: \.self) { c in
                        Pad(title: c.replacingOccurrences(of: "_", with: " "),
                            on: s.clip == c, tint: Theme.dancer, height: 60) {
                            conn.showClip(c)
                        }
                    }
                }
            }
            .padding(16)
        }
    }
}

// MARK: - Look

struct LookView: View {
    @EnvironmentObject var conn: Connection
    @State private var ticker = ""
    /// Fallback for a server that doesn't echo cut_on_drops.
    @State private var cutOnDrops = true

    var body: some View {
        let s = conn.state
        ScrollView {
            VStack(spacing: 14) {
                SectionHeader(title: "Palette")
                FlowLayout(spacing: 8) {
                    ForEach(conn.info.palettes, id: \.self) { p in
                        Chip(title: p, selected: s.palette == p) { conn.set("palette", p) }
                            .accessibilityIdentifier("palette.\(p)")
                            .accessibilityValue(s.palette == p ? "selected" : "")
                    }
                }
                SectionHeader(title: "Effect")
                FlowLayout(spacing: 8) {
                    ForEach(fxModes, id: \.self) { f in
                        Chip(title: f == "Off" ? "None" : f, selected: s.fx == f) { conn.set("fx", f) }
                    }
                }
                Card {
                    RemoteSlider(title: "Amount", key: "fx_amt", value: s.fxAmt, range: 0...1)
                    RemoteToggle(title: "Auto FX on drops", key: "fx_auto", value: s.fxAuto)
                }
                SectionHeader(title: "Director")
                Card {
                    Text("Phrase length").foregroundStyle(Theme.text)
                    HStack(spacing: 8) {
                        ForEach([4, 8, 16, 32], id: \.self) { n in
                            Chip(title: "\(n) bars", selected: s.phraseBars == n) { conn.set("phrase_bars", n) }
                        }
                    }
                    // Older servers don't echo it: then this toggle
                    // remembers what it last sent.
                    Toggle("Cut on drops", isOn: Binding(get: { s.cutOnDrops ?? cutOnDrops }, set: { v in
                        cutOnDrops = v
                        Haptics.press()
                        conn.set("cut_on_drops", v)
                    }))
                    .tint(Theme.accent)
                    .foregroundStyle(Theme.text)
                    RemoteSlider(title: "Audio latency", key: "latency_ms", value: s.latencyMs, range: 0...200, step: 1,
                                 format: { "\(Int($0)) ms" })
                }
                SectionHeader(title: "Overlays")
                Card {
                    RemoteSlider(title: "Now playing size", key: "np_size", value: s.npSize, range: 0.5...2)
                    RemoteSlider(title: "Logo opacity", key: "brand_opacity", value: s.brandOpacity, range: 0.1...1)
                    RemoteSlider(title: "Ticker speed", key: "ticker_speed", value: s.tickerSpeed, range: 0.3...3)
                    HStack {
                        TextField("Ticker text", text: $ticker)
                            .padding(10)
                            .foregroundStyle(Theme.text)
                            .background(RoundedRectangle(cornerRadius: 10).fill(Theme.bg))
                            .overlay(RoundedRectangle(cornerRadius: 10).strokeBorder(Theme.borderHi))
                            .onSubmit { conn.set("ticker_text", ticker) }
                        Button("Send") {
                            Haptics.press()
                            conn.set("ticker_text", ticker)
                        }
                        .buttonStyle(PadStyle(height: 44))
                        .frame(width: 90)
                    }
                }
            }
            .padding(16)
        }
        .onAppear { if ticker.isEmpty { ticker = conn.state.tickerText } }
    }
}

// MARK: - Timeline

/// Song/timeline transport. With `state.song` (newer servers) there's a
/// scrubber and a lit play button; otherwise seek is absolute m:ss.
struct TransportView: View {
    @EnvironmentObject var conn: Connection
    @State private var minutes = 0
    @State private var seconds = 0
    /// Scrub position while the finger is on the slider.
    @State private var scrub: Double?

    var body: some View {
        let song = conn.state.song
        ScrollView {
            VStack(spacing: 14) {
                if let song {
                    Card {
                        Text(song.name ?? "Song")
                            .font(.headline)
                            .foregroundStyle(Theme.text)
                            .lineLimit(1)
                        if let len = song.len, len > 0 {
                            Slider(value: Binding(get: { scrub ?? min(song.pos, len) }, set: { scrub = $0 }),
                                   in: 0...len) { editing in
                                if !editing, let p = scrub {
                                    conn.transport("seek", pos: p)
                                    DispatchQueue.main.asyncAfter(deadline: .now() + 0.4) { scrub = nil }
                                }
                            }
                            HStack {
                                Text(clock(scrub ?? song.pos))
                                Spacer()
                                Text("-" + clock(max(0, len - (scrub ?? song.pos))))
                            }
                            .font(.system(size: 13, weight: .medium, design: .monospaced))
                            .foregroundStyle(Theme.muted)
                        } else {
                            Text(clock(song.pos)).font(.system(.body, design: .monospaced)).foregroundStyle(Theme.muted)
                        }
                    }
                } else if conn.info.version.isEmpty == false {
                    Text("No song loaded in Trippin's timeline.").foregroundStyle(Theme.muted)
                }
                HStack(spacing: 10) {
                    Pad(title: song?.playing == true ? "Pause" : "Play", on: song?.playing == true, height: 96) {
                        conn.transport("toggle")
                    }
                    Pad(title: "Stop", height: 96) { conn.transport("stop") }
                }
                if song?.len == nil {
                    SectionHeader(title: "Seek")
                    Card {
                        Stepper("\(minutes) min", value: $minutes, in: 0...180).foregroundStyle(Theme.text)
                        Stepper("\(seconds) s", value: $seconds, in: 0...59).foregroundStyle(Theme.text)
                        HStack(spacing: 10) {
                            Pad(title: "To start", height: 48) { conn.transport("seek", pos: 0) }
                            Pad(title: String(format: "Go to %d:%02d", minutes, seconds), height: 48) {
                                conn.transport("seek", pos: Double(minutes * 60 + seconds))
                            }
                        }
                    }
                } else {
                    Pad(title: "Back to start", height: 52) { conn.transport("seek", pos: 0) }
                }
                SectionHeader(title: "Timeline")
                HStack(spacing: 10) {
                    ActionPad(key: "TimelinePlay")
                    ActionPad(key: "TimelineRecord")
                }
                SectionHeader(title: "Sync")
                HStack(spacing: 10) {
                    ActionPad(key: "MarkDownbeat")
                    ActionPad(key: "LatencyDown")
                    ActionPad(key: "LatencyUp")
                }
            }
            .padding(16)
        }
    }

    private func clock(_ t: Double) -> String {
        let s = Int(t.rounded(.down))
        return String(format: "%d:%02d", s / 60, s % 60)
    }
}

/// Wrapping row layout for chips.
struct FlowLayout: Layout {
    var spacing: CGFloat = 8

    func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize {
        let width = proposal.width ?? .infinity
        var x: CGFloat = 0, y: CGFloat = 0, row: CGFloat = 0, widest: CGFloat = 0
        for v in subviews {
            let s = v.sizeThatFits(.unspecified)
            if x > 0, x + s.width > width { y += row + spacing; x = 0; row = 0 }
            x += s.width + spacing
            row = max(row, s.height)
            widest = max(widest, x - spacing)
        }
        return CGSize(width: proposal.width ?? widest, height: y + row)
    }

    func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) {
        var x = bounds.minX, y = bounds.minY, row: CGFloat = 0
        for v in subviews {
            let s = v.sizeThatFits(.unspecified)
            if x > bounds.minX, x + s.width > bounds.maxX { y += row + spacing; x = bounds.minX; row = 0 }
            v.place(at: CGPoint(x: x, y: y), proposal: ProposedViewSize(s))
            x += s.width + spacing
            row = max(row, s.height)
        }
    }
}
