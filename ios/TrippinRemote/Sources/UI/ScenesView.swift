import SwiftUI

/// Every scene as a thumbnail tile: tap = cut to it, long-press = queue it
/// as the next cut.
struct ScenesView: View {
    @EnvironmentObject var conn: Connection
    @State private var filter = ""

    var body: some View {
        let names = conn.info.scenes
        let shown = names.indices.filter { filter.isEmpty || names[$0].localizedCaseInsensitiveContains(filter) }
        VStack(spacing: 0) {
            HStack(spacing: 8) {
                Image(systemName: "magnifyingglass").foregroundStyle(Theme.faint)
                TextField("Filter \(names.count) scenes", text: $filter)
                    .textInputAutocapitalization(.never)
                    .autocorrectionDisabled()
                    .foregroundStyle(Theme.text)
                if !filter.isEmpty {
                    Button { filter = "" } label: { Image(systemName: "xmark.circle.fill").foregroundStyle(Theme.faint) }
                }
            }
            .padding(10)
            .background(RoundedRectangle(cornerRadius: 10).fill(Theme.card))
            .overlay(RoundedRectangle(cornerRadius: 10).strokeBorder(Theme.border))
            .padding(.horizontal, 12)
            .padding(.top, 12)
            Text("Tap to cut · hold to queue next")
                .font(.caption)
                .foregroundStyle(Theme.faint)
                .padding(.vertical, 6)
            ScrollViewReader { proxy in
                ScrollView {
                    LazyVGrid(columns: [GridItem(.adaptive(minimum: 150, maximum: 260), spacing: 10)], spacing: 10) {
                        ForEach(shown, id: \.self) { i in
                            SceneTile(index: i, name: names[i],
                                      heavy: conn.info.heavy.indices.contains(i) && conn.info.heavy[i],
                                      current: conn.state.scene == i,
                                      queued: conn.state.nextScene == i)
                                .id(i)
                        }
                    }
                    .padding(.horizontal, 12)
                    .padding(.bottom, 16)
                }
                .onChange(of: conn.state.scene) { _, i in
                    // Keep the live scene in view when the director cuts.
                    if filter.isEmpty, i >= 0 { withAnimation { proxy.scrollTo(i, anchor: .center) } }
                }
            }
        }
    }
}

struct SceneTile: View {
    @EnvironmentObject var conn: Connection
    @EnvironmentObject var thumbs: ThumbCache
    let index: Int
    let name: String
    let heavy: Bool
    let current: Bool
    let queued: Bool

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            ZStack {
                Rectangle().fill(Theme.bg)
                if let img = thumbs.image(name) {
                    Image(uiImage: img).resizable().scaledToFill()
                } else {
                    ProgressView().tint(Theme.faint)
                }
            }
            .aspectRatio(16 / 9, contentMode: .fit)
            .clipShape(RoundedRectangle(cornerRadius: 10, style: .continuous))
            .overlay(alignment: .topTrailing) {
                if current { badge("LIVE", Theme.accent) } else if queued { badge("NEXT", Theme.next) }
            }
            HStack(spacing: 4) {
                Text(name)
                    .font(.system(size: 13, weight: current ? .bold : .medium))
                    .foregroundStyle(current ? Theme.accent : Theme.text)
                    .lineLimit(1)
                if heavy {
                    Text("3D").font(.system(size: 9, weight: .bold)).foregroundStyle(Theme.faint)
                }
            }
        }
        .padding(6)
        .background(RoundedRectangle(cornerRadius: 14, style: .continuous).fill(current ? Theme.accentSel.opacity(0.5) : Theme.card))
        .overlay(
            RoundedRectangle(cornerRadius: 14, style: .continuous)
                .strokeBorder(current ? Theme.accent : (queued ? Theme.next : Theme.border), lineWidth: current || queued ? 2 : 1)
        )
        .contentShape(Rectangle())
        // Hold wins over tap: a finger held 0.35 s queues and never also cuts.
        .gesture(
            LongPressGesture(minimumDuration: 0.35)
                .exclusively(before: TapGesture())
                .onEnded { g in
                    switch g {
                    case .first:
                        Haptics.strong()
                        conn.queueNext(name)
                    case .second:
                        Haptics.press()
                        conn.goTo(name)
                    }
                }
        )
        // Ask for the thumbnail when the tile shows — and again after a
        // reconnect, since requests in flight die with the socket.
        .task(id: conn.isConnected) {
            if conn.isConnected { thumbs.want(name) }
        }
        .accessibilityElement(children: .ignore)
        .accessibilityAddTraits(.isButton)
        .accessibilityIdentifier("scene.\(name)")
        .accessibilityLabel(name)
        .accessibilityValue([current ? "live" : nil, queued ? "next" : nil, thumbs.image(name) != nil ? "thumb" : nil]
            .compactMap { $0 }.joined(separator: ","))
        .accessibilityAction(named: "Queue next") { conn.queueNext(name) }
    }

    private func badge(_ t: String, _ c: Color) -> some View {
        Text(t)
            .font(.system(size: 10, weight: .heavy))
            .foregroundStyle(.black)
            .padding(.horizontal, 6).padding(.vertical, 3)
            .background(Capsule().fill(c))
            .padding(6)
    }
}
