// Faces: one initial on a disc, and a ring that says where the person is (the room board, v5).
// Solid water with the room, haze while the room is paused, an arc filling toward the 20 s cap
// while the room waits for them, and no ring with the face dimmed when they aren't with the room.

import KaeriDesign
import KaeriKit
import KaeriStore
import SwiftUI

/// The seconds the room's words count, on a one-second tick of this device's clock.
struct RoomClock<Content: View>: View {
    @ViewBuilder let content: (Int64) -> Content

    var body: some View {
        if let still = RoomModel.stillAt {
            content(still)
        } else {
            TimelineView(.periodic(from: .now, by: 1)) { context in
                content(Int64((context.date.timeIntervalSince1970 * 1000).rounded()))
            }
        }
    }
}

extension Int64 {
    /// Now, in epoch ms (a screenshot's still moment when one is set).
    @MainActor static var nowMs: Int64 { RoomModel.stillAt ?? Int64((Date.now.timeIntervalSince1970 * 1000).rounded()) }
}

/// A face of `disc` points, with its ring around it.
struct RoomFace: View {
    let name: String
    let look: RoomFaceLook
    let disc: CGFloat

    private var stroke: CGFloat { disc >= 48 ? 3 : disc >= 30 ? 2 : 1.75 }
    private var gap: CGFloat { disc >= 48 ? 3 : 2 }
    var size: CGFloat { disc + 2 * (stroke + gap) }

    var body: some View {
        ZStack {
            ring
            Text(String(name.prefix(1)))
                .font(.system(size: (disc * 0.42).rounded(), weight: .semibold))
                .foregroundStyle(.silk)
                .frame(width: disc, height: disc)
                .background(Palette.raisedSelected, in: .circle)
                .opacity(look.dim)
                .animation(.fadeIn, value: look.dim)
        }
        .frame(width: size, height: size)
        .overlay(alignment: .bottomTrailing) {
            if look.pauseBadge {
                let badge = max(12, (disc * 0.42).rounded())
                Image(systemName: "pause.fill")
                    .font(.system(size: badge * 0.5, weight: .bold))
                    .foregroundStyle(.silk)
                    .frame(width: badge, height: badge)
                    .background(Palette.ground, in: .circle)
                    .offset(x: 1, y: 1)
            }
        }
        .overlay(alignment: .bottomTrailing) {
            if look.error {
                let dot = max(8, (disc * 0.28).rounded())
                Circle()
                    .fill(Palette.vermilion)
                    .frame(width: dot, height: dot)
                    .overlay(Circle().strokeBorder(Palette.ground, lineWidth: 2).padding(-2))
                    .padding(stroke)
            }
        }
        .accessibilityHidden(true)
    }

    @ViewBuilder
    private var ring: some View {
        let inset = stroke / 2
        switch look.ring {
        case .with:
            Circle().inset(by: inset).stroke(Palette.water, lineWidth: stroke)
        case .paused:
            Circle().inset(by: inset).stroke(Palette.haze, lineWidth: stroke)
        case .waiting(let part):
            ZStack {
                Circle().inset(by: inset).stroke(Palette.rule, lineWidth: stroke)
                Circle().inset(by: inset)
                    .trim(from: 0, to: part)
                    .stroke(Palette.silk, style: StrokeStyle(lineWidth: stroke, lineCap: .round))
                    .rotationEffect(.degrees(-90))
                    .animation(.linear(duration: 1), value: part)
            }
        case .none:
            EmptyView()
        }
    }
}

/// The "+3" face that opens the full list.
struct RoomMoreFace: View {
    let label: String
    let disc: CGFloat

    var body: some View {
        let stroke: CGFloat = disc >= 48 ? 3 : disc >= 30 ? 2 : 1.75
        let gap: CGFloat = disc >= 48 ? 3 : 2
        Text(label)
            .font(.system(size: (disc * 0.36).rounded(), weight: .semibold))
            .foregroundStyle(.haze)
            .frame(width: disc, height: disc)
            .background(Palette.raised, in: .circle)
            .overlay(Circle().strokeBorder(Palette.rule, lineWidth: 1))
            .frame(width: disc + 2 * (stroke + gap), height: disc + 2 * (stroke + gap))
            .accessibilityHidden(true)
    }
}

/// Up to four faces in order, then "+N" for the rest.
enum RoomFaces {
    static func capped(_ people: [RoomMember]) -> (shown: [RoomMember], rest: [RoomMember]) {
        guard people.count > 4 else { return (people, []) }
        return (Array(people.prefix(3)), Array(people.dropFirst(3)))
    }

    static func moreWord(_ rest: [RoomMember]) -> String {
        let n = rest.filter { $0.state != .watching }.count
        return n > 0 ? RoomCopy.notWatching(n) : RoomCopy.allWatching
    }
}

/// The full screen strip: faces in a glass capsule with the room's line, and Carry on when it is
/// offered. It shows while anyone isn't watching, and for 8 s after any change.
struct RoomStrip: View {
    let room: RoomModel
    var disc: CGFloat = 24
    var gap: CGFloat = 6
    var touch = false

    var body: some View {
        RoomClock { now in
            let status = room.status(now: now)
            let (shown, rest) = RoomFaces.capped(room.people)
            let carry: String? = if case .carryOn(let who) = status.action { RoomCopy.carryOnWithout(who) } else { nil }
            let height = carry != nil && touch ? 52 : disc + 16
            let size: CGFloat = disc >= 28 ? 14 : 13
            HStack(spacing: gap) {
                ForEach(shown) { member in
                    RoomFace(name: member.name, look: room.look(member, now: now), disc: disc)
                        .accessibilityElement()
                        .accessibilityLabel("\(member.name), \(room.word(member, now: now))")
                }
                if !rest.isEmpty {
                    RoomMoreFace(label: "+\(rest.count)", disc: disc)
                }
                HStack(spacing: 0) {
                    Text(status.title).fontWeight(.semibold)
                    if status.waits, carry == nil, let since = room.view?.waiting.map(\.sinceMs).min() {
                        Text(" · \(RoomModel.seconds(since: since, now: now)) s")
                            .foregroundStyle(.haze)
                            .monospacedDigit()
                    }
                }
                .font(.system(size: size))
                .lineLimit(1)
                .contentTransition(.opacity)
                .padding(.leading, 4)
                .padding(.trailing, 6)
                if let carry {
                    let button: CGFloat = touch ? 44 : disc + 6
                    Button(carry) { room.carryOn() }
                        .buttonStyle(RoomButtonStyle(kind: .primary, height: button, size: size, padding: touch ? 18 : 14))
                        .padding(.trailing, touch ? -2 : -4)
                        .transition(.opacity)
                }
            }
            .padding(.horizontal, gap + 2)
            .frame(height: height)
            .background(.ultraThinMaterial.opacity(0.9), in: .capsule)
            .background(Palette.glass.opacity(0.6), in: .capsule)
            .overlay(Capsule().strokeBorder(Palette.glassOverArtEdge, lineWidth: 1))
            .shadow(color: .black.opacity(0.35), radius: 15, y: 10)
            .foregroundStyle(.silk)
            .animation(.fadeIn, value: carry)
        }
    }
}

/// The room's buttons: amber for the one thing to press, quiet glass for the rest.
struct RoomButtonStyle: ButtonStyle {
    enum Kind { case primary, secondary, glass }
    var kind: Kind = .secondary
    var height: CGFloat = 32
    var size: CGFloat = 13
    var padding: CGFloat = 14
    var wide = false
    @Environment(\.isEnabled) private var enabled

    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .font(.system(size: size, weight: .semibold))
            .lineLimit(1)
            .padding(.horizontal, padding)
            .frame(maxWidth: wide ? .infinity : nil)
            .frame(height: height)
            .foregroundStyle(kind == .primary ? Palette.ground : Palette.silk)
            .background {
                switch kind {
                case .primary: Capsule().fill(Palette.amber)
                case .secondary: Capsule().fill(Palette.silk.opacity(0.12)).overlay(Capsule().strokeBorder(Palette.glassEdge, lineWidth: 1))
                case .glass: Capsule().fill(Palette.glass).overlay(Capsule().strokeBorder(Palette.glassOverArtEdge, lineWidth: 1))
                }
            }
            .opacity(configuration.isPressed ? 0.8 : enabled ? 1 : 0.5)
            .contentShape(.capsule)
    }
}
