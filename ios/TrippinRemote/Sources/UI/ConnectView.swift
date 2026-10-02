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

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 18) {
                header
                if case .badPin = conn.status {
                    banner("Wrong PIN — check Trippin's Settings → Remote card.", Theme.danger, Theme.dangerBg)
                } else if case .lost(let reason, _) = conn.status, conn.target != nil {
                    banner("Can't reach \(conn.target!.key): \(reason). Retrying…", Theme.warn, Theme.warnBg)
                }
                if let t = pending { pinCard(t) }
                found
                manual
                Text("Turn the remote on in Trippin: Settings → Remote. The card there shows this Mac/PC's address and the pairing PIN.")
                    .font(.footnote)
                    .foregroundStyle(Theme.faint)
            }
            .padding(20)
            .frame(maxWidth: 560)
            .frame(maxWidth: .infinity)
        }
        .background(Theme.bg.ignoresSafeArea())
        .onAppear { discovery.start() }
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
                Button {
                    choose(.bonjour(name: s.name, endpoint: s.endpoint))
                } label: {
                    HStack(spacing: 12) {
                        Image(systemName: "display")
                            .font(.title3)
                            .foregroundStyle(Theme.accent)
                        VStack(alignment: .leading, spacing: 2) {
                            Text(s.name).font(.headline).foregroundStyle(Theme.text)
                            Text(PinStore.get(s.name) != nil ? "Paired" : "Needs PIN")
                                .font(.caption).foregroundStyle(Theme.muted)
                        }
                        Spacer()
                        Image(systemName: "chevron.right").foregroundStyle(Theme.faint)
                    }
                    .padding(14)
                    .background(RoundedRectangle(cornerRadius: 14).fill(Theme.card))
                    .overlay(RoundedRectangle(cornerRadius: 14).strokeBorder(Theme.border))
                }
                .buttonStyle(.plain)
            }
        }
    }

    private var manual: some View {
        VStack(alignment: .leading, spacing: 10) {
            SectionHeader(title: "Or type its address")
            Card {
                HStack(spacing: 10) {
                    field("192.168.1.20", text: $manualHost, keyboard: .numbersAndPunctuation)
                    field("9138", text: $manualPort, keyboard: .numberPad)
                        .frame(width: 90)
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
            }
        }
    }

    private func pinCard(_ t: Target) -> some View {
        Card {
            Text("PIN for \(t.key)").font(.headline).foregroundStyle(Theme.text)
            field("PIN", text: $pin, keyboard: .numberPad)
                .focused($pinFocused)
                .font(.system(size: 28, weight: .bold, design: .monospaced))
                .onSubmit { go(t) }
            HStack {
                Button("Cancel") { pending = nil; conn.disconnect() }
                    .buttonStyle(PadStyle(height: 48))
                Button("Connect") { go(t) }
                    .buttonStyle(PadStyle(on: !pin.isEmpty, height: 48))
                    .disabled(pin.isEmpty)
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

    /// A remembered PIN connects straight away; otherwise ask for one.
    private func choose(_ t: Target) {
        Haptics.press()
        if let saved = PinStore.get(t.key) {
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
        pinFocused = false
        pending = nil
        conn.connect(t, pin: p)
    }
}
