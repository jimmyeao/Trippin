import Network
import SwiftUI

/// Pick a Trippin (Bonjour or typed address), enter its PIN, connect.
struct ConnectView: View {
    @EnvironmentObject var conn: Connection
    @StateObject private var discovery = Discovery()
    @AppStorage("manualHost") private var manualHost = ""
    @AppStorage("manualPort") private var manualPort = "9138"
    @State private var pending: Target?
    @State private var pin = ""
    @FocusState private var pinFocused: Bool
    @State private var saved: [SavedServer] = []
    /// The server a Forget confirmation is up for.
    @State private var forgetting: String?

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 18) {
                header
                if case .badPin = conn.status {
                    banner("Wrong PIN — check Trippin's Settings → Remote card.", Theme.danger, Theme.dangerBg)
                        .accessibilityIdentifier("badPin")
                } else if case .lost(let reason, _) = conn.status, conn.target != nil {
                    banner("Can't reach \(conn.target!.key): \(reason). Retrying…", Theme.warn, Theme.warnBg)
                }
                if let t = pending { pinCard(t) }
                found
                savedList
                manual
                demo
                Text("Turn the remote on in Trippin: Settings → Remote. The card there shows this Mac/PC's address and the pairing PIN.")
                    .font(.footnote)
                    .foregroundStyle(Theme.faint)
            }
            .padding(20)
            .frame(maxWidth: 560)
            .frame(maxWidth: .infinity)
        }
        .background(Theme.bg.ignoresSafeArea())
        .onAppear {
            discovery.start()
            saved = Saved.all()
        }
        .confirmationDialog(
            "Forget \(forgetting ?? "this server")?",
            isPresented: Binding(get: { forgetting != nil }, set: { if !$0 { forgetting = nil } }),
            titleVisibility: .visible
        ) {
            Button("Forget", role: .destructive) {
                if let key = forgetting { forget(key) }
            }
            Button("Cancel", role: .cancel) {}
        } message: {
            Text("Its saved PIN is deleted. You'll need the PIN from Trippin's Settings → Remote card to connect again.")
        }
        .onDisappear { discovery.stop() }
        .onChange(of: conn.status) { _, s in
            if s == .badPin {
                Haptics.error()
                pending = conn.target
                pin = ""
                pinFocused = true
            }
        }
    }

    private var header: some View {
        VStack(alignment: .leading, spacing: 4) {
            Text("Trippin Remote")
                .font(.system(size: 32, weight: .bold, design: .rounded))
                .foregroundStyle(Theme.text)
            Text("Control your visuals from the booth")
                .foregroundStyle(Theme.muted)
        }
        .padding(.top, 20)
    }

    private var found: some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack {
                SectionHeader(title: "On this network")
                ProgressView().tint(Theme.faint).scaleEffect(0.8)
            }
            if let e = discovery.error {
                Text(e).font(.footnote).foregroundStyle(Theme.warn)
            }
            if discovery.servers.isEmpty {
                Card {
                    Text("Looking for Trippin…").foregroundStyle(Theme.muted)
                }
            }
            ForEach(discovery.servers) { s in
                let paired = saved.contains { $0.key == s.name }
                ServerRow(id: "server.\(s.name)", title: s.name, subtitle: paired ? "Paired" : "Needs PIN", icon: "display",
                          onForget: paired ? { forgetting = s.name } : nil) {
                    choose(.bonjour(name: s.name, endpoint: s.endpoint))
                }
            }
        }
    }

    /// Servers connected to before that aren't on the network list right
    /// now — a saved Bonjour name still resolves if it comes back.
    @ViewBuilder private var savedList: some View {
        let visible = Set(discovery.servers.map(\.name))
        let rows = saved.filter { !visible.contains($0.key) }
        if !rows.isEmpty {
            VStack(alignment: .leading, spacing: 10) {
                SectionHeader(title: "Saved")
                ForEach(rows) { s in
                    ServerRow(id: "saved.\(s.key)", title: s.key,
                              subtitle: s.kind == .manual ? "Typed address · paired" : "Not seen on this network right now",
                              icon: s.kind == .manual ? "network" : "display",
                              onForget: { forgetting = s.key }) {
                        choose(target(for: s))
                    }
                }
            }
        }
    }

    private var demo: some View {
        Button {
            Haptics.press()
            endEditing()
            conn.connect(.demo, pin: "")
        } label: {
            Label("No Trippin handy? Try the demo", systemImage: "sparkles")
                .font(.callout.weight(.semibold))
                .foregroundStyle(Theme.accent)
                .frame(maxWidth: .infinity, minHeight: 44)
        }
        .accessibilityIdentifier("demo")
    }

    private func target(for s: SavedServer) -> Target {
        switch s.kind {
        case .manual:
            return .manual(host: s.host ?? "", port: s.port ?? 9138)
        case .bonjour:
            return .bonjour(name: s.key, endpoint: .service(name: s.key, type: "_trippin._tcp", domain: "local.", interface: nil))
        }
    }

    private func forget(_ key: String) {
        Haptics.success()
        Saved.forget(key)
        if pending?.key == key { pending = nil }
        // A forgotten typed address shouldn't linger in the field either.
        if key == "\(manualHost):\(manualPort)" { manualHost = "" }
        saved = Saved.all()
    }

    private var manual: some View {
        VStack(alignment: .leading, spacing: 10) {
            SectionHeader(title: "Or type its address")
            Card {
                HStack(spacing: 10) {
                    field("192.168.1.20", text: $manualHost, keyboard: .numbersAndPunctuation)
                        .accessibilityIdentifier("host")
                    field("9138", text: $manualPort, keyboard: .numberPad)
                        .frame(width: 90)
                        .accessibilityIdentifier("port")
                }
                Button {
                    let host = manualHost.trimmingCharacters(in: .whitespaces)
                    guard !host.isEmpty else { return }
                    choose(.manual(host: host, port: UInt16(manualPort) ?? 9138))
                } label: {
                    Text("Connect").frame(maxWidth: .infinity)
                }
                .buttonStyle(PadStyle(height: 48))
                .disabled(manualHost.trimmingCharacters(in: .whitespaces).isEmpty)
                .accessibilityIdentifier("connect")
            }
        }
    }

    private func pinCard(_ t: Target) -> some View {
        Card {
            Text("PIN for \(t.key)").font(.headline).foregroundStyle(Theme.text)
            field("PIN", text: $pin, keyboard: .numberPad)
                .focused($pinFocused)
                .accessibilityIdentifier("pin")
                .font(.system(size: 28, weight: .bold, design: .monospaced))
                .onSubmit { go(t) }
            HStack {
                Button("Cancel") { pending = nil; conn.disconnect() }
                    .buttonStyle(PadStyle(height: 48))
                Button("Connect") { go(t) }
                    .buttonStyle(PadStyle(on: !pin.isEmpty, height: 48))
                    .disabled(pin.isEmpty)
                    .accessibilityIdentifier("pinConnect")
            }
        }
    }

    private func field(_ placeholder: String, text: Binding<String>, keyboard: UIKeyboardType) -> some View {
        TextField(placeholder, text: text)
            .keyboardType(keyboard)
            .textInputAutocapitalization(.never)
            .autocorrectionDisabled()
            .padding(12)
            .foregroundStyle(Theme.text)
            .background(RoundedRectangle(cornerRadius: 10).fill(Theme.bg))
            .overlay(RoundedRectangle(cornerRadius: 10).strokeBorder(Theme.borderHi))
    }

    private func banner(_ text: String, _ fg: Color, _ bg: Color) -> some View {
        Text(text)
            .font(.callout.weight(.medium))
            .foregroundStyle(fg)
            .padding(12)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(RoundedRectangle(cornerRadius: 10).fill(bg))
    }

    /// Drop text focus before this screen can be replaced by the control
    /// UI: tearing down a still-focused field trips a UIKit focus-system
    /// assertion on iPad (crash seen with a remembered PIN).
    private func endEditing() {
        pinFocused = false
        UIApplication.shared.sendAction(#selector(UIResponder.resignFirstResponder), to: nil, from: nil, for: nil)
    }

    /// A remembered PIN connects straight away; otherwise ask for one.
    private func choose(_ t: Target) {
        Haptics.press()
        if let saved = PinStore.get(t.key) {
            endEditing()
            pending = nil
            conn.connect(t, pin: saved)
        } else {
            pending = t
            pin = ""
            pinFocused = true
        }
    }

    private func go(_ t: Target) {
        let p = pin.trimmingCharacters(in: .whitespaces)
        guard !p.isEmpty else { return }
        endEditing()
        pending = nil
        conn.connect(t, pin: p)
    }
}

/// A connect-screen row: tap connects; the trailing menu forgets a saved
/// server (rows sit in a ScrollView, so there are no swipe actions).
struct ServerRow: View {
    let id: String
    let title: String
    let subtitle: String
    let icon: String
    var onForget: (() -> Void)?
    let action: () -> Void

    var body: some View {
        HStack(spacing: 0) {
            Button(action: action) {
                HStack(spacing: 12) {
                    Image(systemName: icon)
                        .font(.title3)
                        .foregroundStyle(Theme.accent)
                        .frame(width: 28)
                    VStack(alignment: .leading, spacing: 2) {
                        Text(title).font(.headline).foregroundStyle(Theme.text).lineLimit(1)
                        Text(subtitle).font(.caption).foregroundStyle(Theme.muted).lineLimit(1)
                    }
                    Spacer(minLength: 8)
                    if onForget == nil {
                        Image(systemName: "chevron.right").foregroundStyle(Theme.faint)
                    }
                }
                .padding(.vertical, 14)
                .padding(.leading, 14)
                .padding(.trailing, onForget == nil ? 14 : 4)
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .accessibilityIdentifier(id)
            if let onForget {
                Menu {
                    Button(role: .destructive, action: onForget) {
                        Label("Forget", systemImage: "trash")
                    }
                } label: {
                    Image(systemName: "ellipsis")
                        .font(.body.weight(.semibold))
                        .foregroundStyle(Theme.muted)
                        .frame(width: 48, height: 48)
                        .contentShape(Rectangle())
                }
                .accessibilityLabel("Options for \(title)")
                .accessibilityIdentifier("options.\(title)")
            }
        }
        .background(RoundedRectangle(cornerRadius: 14).fill(Theme.card))
        .overlay(RoundedRectangle(cornerRadius: 14).strokeBorder(Theme.border))
        .contextMenu {
            if let onForget {
                Button(role: .destructive, action: onForget) { Label("Forget", systemImage: "trash") }
            }
        }
    }
}
