import SwiftUI
import UIKit

/// Trippin's panel palette (`src/ui_theme.rs`), so the remote reads as the
/// same product: near-black grounds, one cyan accent.
enum Theme {
    static let bg = Color(hex: 0x0E0F11)
    static let panel = Color(hex: 0x121416)
    static let card = Color(hex: 0x14161A)
    static let raised = Color(hex: 0x1F2226)
    static let hover = Color(hex: 0x262A2F)
    static let border = Color(hex: 0x24272B)
    static let borderHi = Color(hex: 0x2A2E33)
    static let text = Color(hex: 0xE6E7E9)
    static let muted = Color(hex: 0x8B9096)
    static let faint = Color(hex: 0x6C7177)
    static let accent = Color(hex: 0x3FB0D8)
    static let accentSel = Color(hex: 0x1A4D61)
    static let next = Color(hex: 0x8A7FF0)      // LANE_SCENE: the queued scene
    static let dancer = Color(hex: 0xE25FA8)    // LANE_DANCER
    static let warn = Color(hex: 0xF0A650)
    static let warnBg = Color(hex: 0x33230F)
    static let danger = Color(hex: 0xF08A84)
    static let dangerBg = Color(hex: 0x3A1716)
    static let good = Color(hex: 0x5FD39A)
    static let calm = Color(hex: 0xA9A6F0)      // BREAKDOWN
}

extension Color {
    init(hex: UInt32) {
        self.init(.sRGB,
                  red: Double((hex >> 16) & 0xFF) / 255,
                  green: Double((hex >> 8) & 0xFF) / 255,
                  blue: Double(hex & 0xFF) / 255)
    }
}

enum Haptics {
    private static let tap = UIImpactFeedbackGenerator(style: .medium)
    private static let heavy = UIImpactFeedbackGenerator(style: .heavy)
    private static let note = UINotificationFeedbackGenerator()
    static func press() { tap.impactOccurred() }
    static func strong() { heavy.impactOccurred() }
    static func success() { note.notificationOccurred(.success) }
    static func error() { note.notificationOccurred(.error) }
}

/// A big booth pad: high-contrast label, lit when `on`, haptic on press.
struct PadStyle: ButtonStyle {
    var on = false
    var tint: Color = Theme.accent
    var height: CGFloat = 72

    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .font(.system(size: 17, weight: .semibold, design: .rounded))
            .multilineTextAlignment(.center)
            .lineLimit(2)
            .minimumScaleFactor(0.7)
            .foregroundStyle(on ? Color.black : Theme.text)
            .frame(maxWidth: .infinity, minHeight: height)
            .padding(.horizontal, 8)
            .background(
                RoundedRectangle(cornerRadius: 14, style: .continuous)
                    .fill(on ? tint : (configuration.isPressed ? Theme.hover : Theme.raised))
            )
            .overlay(
                RoundedRectangle(cornerRadius: 14, style: .continuous)
                    .strokeBorder(configuration.isPressed ? tint : Theme.borderHi, lineWidth: configuration.isPressed ? 2 : 1)
            )
            .scaleEffect(configuration.isPressed ? 0.97 : 1)
            .animation(.easeOut(duration: 0.08), value: configuration.isPressed)
    }
}

/// A booth pad: a button in `PadStyle` with a haptic, optional subtitle.
struct Pad: View {
    let title: String
    var subtitle: String? = nil
    var on = false
    var tint: Color = Theme.accent
    var height: CGFloat = 72
    let action: () -> Void

    var body: some View {
        Button {
            Haptics.press()
            action()
        } label: {
            VStack(spacing: 2) {
                Text(title)
                if let subtitle {
                    Text(subtitle)
                        .font(.system(size: 12, weight: .medium))
                        .opacity(0.7)
                }
            }
        }
        .buttonStyle(PadStyle(on: on, tint: tint, height: height))
        .accessibilityValue(on ? "on" : "off")
    }
}

struct SectionHeader: View {
    let title: String
    var body: some View {
        Text(title.uppercased())
            .font(.system(size: 12, weight: .semibold))
            .tracking(1.2)
            .foregroundStyle(Theme.faint)
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(.top, 6)
    }
}

struct Card<Content: View>: View {
    @ViewBuilder var content: Content
    var body: some View {
        VStack(alignment: .leading, spacing: 12) { content }
            .padding(14)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(RoundedRectangle(cornerRadius: 14, style: .continuous).fill(Theme.card))
            .overlay(RoundedRectangle(cornerRadius: 14, style: .continuous).strokeBorder(Theme.border))
    }
}

/// Selectable chip (palettes, FX modes, phrase lengths).
struct Chip: View {
    let title: String
    let selected: Bool
    var tint: Color = Theme.accent
    let action: () -> Void
    var body: some View {
        Button {
            Haptics.press()
            action()
        } label: {
            Text(title)
                .font(.system(size: 15, weight: .semibold, design: .rounded))
                .lineLimit(1)
                .padding(.horizontal, 14)
                .frame(minHeight: 44)
                .foregroundStyle(selected ? Color.black : Theme.text)
                .background(Capsule().fill(selected ? tint : Theme.raised))
                .overlay(Capsule().strokeBorder(selected ? tint : Theme.borderHi))
        }
        .buttonStyle(.plain)
    }
}
