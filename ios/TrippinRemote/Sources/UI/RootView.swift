import SwiftUI

@main
struct TrippinRemoteApp: App {
    @StateObject private var conn = Connection()

    init() {
        // UI tests launch with -uitestReset: no saved PINs or addresses.
        if ProcessInfo.processInfo.arguments.contains("-uitestReset") {
            PinStore.removeAll()
            if let id = Bundle.main.bundleIdentifier {
                UserDefaults.standard.removePersistentDomain(forName: id)
            }
        }
    }

    var body: some Scene {
        WindowGroup {
            RootView()
                .environmentObject(conn)
                .environmentObject(conn.thumbs)
                .preferredColorScheme(.dark)
                .tint(Theme.accent)
        }
    }
}

struct RootView: View {
    @EnvironmentObject var conn: Connection
    var body: some View {
        ZStack {
            Theme.bg.ignoresSafeArea()
            if conn.hasSession {
                MainView()
            } else {
                ConnectView()
                if conn.status == .connecting {
                    connecting
                }
            }
        }
        .animation(.easeInOut(duration: 0.2), value: conn.hasSession)
    }

    private var connecting: some View {
        VStack(spacing: 14) {
            ProgressView().controlSize(.large).tint(Theme.accent)
            Text("Connecting to \(conn.target?.key ?? "Trippin")…").foregroundStyle(Theme.text)
            Button("Cancel") { conn.disconnect() }.buttonStyle(PadStyle(height: 44)).frame(width: 140)
        }
        .padding(28)
        .background(RoundedRectangle(cornerRadius: 18).fill(Theme.panel))
        .overlay(RoundedRectangle(cornerRadius: 18).strokeBorder(Theme.borderHi))
    }
}

enum Page: String, CaseIterable, Identifiable {
    case perform = "Perform", scenes = "Scenes", dancer = "Dancer", look = "Look", timeline = "Timeline"
    var id: String { rawValue }
    var icon: String {
        switch self {
        case .perform: return "square.grid.2x2.fill"
        case .scenes: return "photo.stack"
        case .dancer: return "figure.dance"
        case .look: return "paintpalette"
        case .timeline: return "play.rectangle"
        }
    }
}

/// iPhone (compact width): tabs. iPad / wide: the scene grid alongside a
/// control column whose page is picked from a segmented bar.
struct MainView: View {
    @Environment(\.horizontalSizeClass) private var hsc
    @State private var page: Page = .perform
    @State private var side: Page = .perform

    var body: some View {
        VStack(spacing: 0) {
            StatusBar()
            if hsc == .regular {
                wide
            } else {
                tabs
            }
        }
        .background(Theme.bg.ignoresSafeArea())
    }

    private var tabs: some View {
        TabView(selection: $page) {
            ForEach(Page.allCases) { p in
                pageView(p)
                    .tabItem { Label(p.rawValue, systemImage: p.icon) }
                    .tag(p)
            }
        }
    }

    private var wide: some View {
        GeometryReader { geo in
            HStack(spacing: 0) {
                ScenesView()
                    .frame(width: geo.size.width * 0.56)
                Rectangle().fill(Theme.border).frame(width: 1)
                VStack(spacing: 0) {
                    Picker("Page", selection: $side) {
                        ForEach(Page.allCases.filter { $0 != .scenes }) { p in
                            Text(p.rawValue).tag(p)
                        }
                    }
                    .pickerStyle(.segmented)
                    .padding(12)
                    pageView(side)
                }
            }
        }
    }

    @ViewBuilder private func pageView(_ p: Page) -> some View {
        switch p {
        case .perform: PerformView()
        case .scenes: ScenesView()
        case .dancer: DancerView()
        case .look: LookView()
        case .timeline: TransportView()
        }
    }
}

/// Always-visible show state: link, scene → next, BPM with a beat strip,
/// phrase progress, now playing.
struct StatusBar: View {
    @EnvironmentObject var conn: Connection
    @State private var confirmLeave = false

    var body: some View {
        let s = conn.state
        VStack(spacing: 8) {
            HStack(alignment: .center, spacing: 12) {
                link
                VStack(alignment: .leading, spacing: 2) {
                    Text(s.sceneName.isEmpty ? "—" : s.sceneName)
                        .font(.system(size: 20, weight: .bold, design: .rounded))
                        .foregroundStyle(Theme.text)
                        .lineLimit(1)
                    HStack(spacing: 6) {
                        if let n = s.nextSceneName {
                            Text("next \(n)").foregroundStyle(Theme.next)
                        }
                        if !s.mode.isEmpty { Text(s.mode).foregroundStyle(Theme.muted) }
                        if s.blackout { Text("BLACKOUT").foregroundStyle(Theme.danger).bold() }
                        if s.recOn { Text("● REC").foregroundStyle(Theme.danger).bold() }
                    }
                    .font(.system(size: 13, weight: .medium))
                    .lineLimit(1)
                }
                Spacer(minLength: 8)
                VStack(alignment: .trailing, spacing: 4) {
                    Text(s.bpm > 0 ? String(format: "%.1f", s.bpm) : "—")
                        .font(.system(size: 26, weight: .bold, design: .monospaced))
                        .foregroundStyle(s.silent ? Theme.faint : Theme.text)
                    BeatStrip(beat: s.beatInBar, calm: s.calm > 0.5, silent: s.silent)
                }
                Menu {
                    Text("\(conn.target?.key ?? "") · Trippin \(conn.info.version)")
                    Button(role: .destructive) { conn.disconnect() } label: {
                        Label("Disconnect", systemImage: "xmark.circle")
                    }
                } label: {
                    Image(systemName: "ellipsis.circle").font(.title2).foregroundStyle(Theme.muted)
                        .frame(width: 44, height: 44)
                }
            }
            if s.barsTotal > 0 {
                PhraseBar(bar: s.barInScene, total: s.barsTotal)
            }
            if let np = s.nowPlaying {
                HStack(spacing: 6) {
                    Image(systemName: "music.note").foregroundStyle(Theme.accent)
                    Text(np).foregroundStyle(Theme.muted).lineLimit(1)
                    Spacer()
                }
                .font(.system(size: 13, weight: .medium))
            }
            if case .lost(let reason, _) = conn.status {
                Text("Reconnecting — \(reason)")
                    .font(.system(size: 13, weight: .semibold))
                    .foregroundStyle(Theme.warn)
                    .frame(maxWidth: .infinity)
                    .padding(.vertical, 6)
                    .background(RoundedRectangle(cornerRadius: 8).fill(Theme.warnBg))
            } else if let e = conn.lastError {
                Text(e)
                    .font(.system(size: 12, weight: .medium))
                    .foregroundStyle(Theme.danger)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .onTapGesture { conn.lastError = nil }
            }
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 10)
        .background(Theme.panel)
        .overlay(alignment: .bottom) { Rectangle().fill(Theme.border).frame(height: 1) }
    }

    private var link: some View {
        let (c, label): (Color, String) = {
            switch conn.status {
            case .connected: return (Theme.good, "Live")
            case .connecting: return (Theme.warn, "…")
            case .lost: return (Theme.warn, "Lost")
            case .badPin: return (Theme.danger, "PIN")
            case .idle: return (Theme.faint, "Off")
            }
        }()
        return VStack(spacing: 3) {
            Circle().fill(c).frame(width: 10, height: 10)
                .shadow(color: c.opacity(0.8), radius: conn.isConnected ? 4 : 0)
            Text(label).font(.system(size: 10, weight: .bold)).foregroundStyle(c)
        }
        .frame(width: 34)
        .accessibilityElement(children: .ignore)
        .accessibilityIdentifier("link")
        .accessibilityValue(label)
    }
}

/// Four beat cells; the current one lit (lavender in a breakdown).
struct BeatStrip: View {
    let beat: Int
    let calm: Bool
    let silent: Bool
    var body: some View {
        HStack(spacing: 4) {
            ForEach(0..<4, id: \.self) { i in
                RoundedRectangle(cornerRadius: 2)
                    .fill(!silent && i == beat % 4 ? (calm ? Theme.calm : Theme.accent) : Theme.raised)
                    .frame(width: 16, height: 6)
            }
        }
        .animation(.linear(duration: 0.05), value: beat)
    }
}

/// Bars played of the scene's phrase — the cut comes when it fills.
struct PhraseBar: View {
    let bar: Int
    let total: Int
    var body: some View {
        HStack(spacing: 8) {
            GeometryReader { g in
                ZStack(alignment: .leading) {
                    Capsule().fill(Theme.raised)
                    Capsule().fill(Theme.accent.opacity(0.8))
                        .frame(width: g.size.width * min(1, Double(bar) / Double(max(total, 1))))
                }
            }
            .frame(height: 4)
            Text("bar \(min(bar + 1, total))/\(total)")
                .font(.system(size: 11, weight: .medium, design: .monospaced))
                .foregroundStyle(Theme.faint)
        }
    }
}
