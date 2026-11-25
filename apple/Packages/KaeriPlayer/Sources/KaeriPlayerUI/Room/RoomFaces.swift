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

/// A face of `disc` points for a room member, with its ring around it.
struct RoomFace: View {
    let name: String
    let look: RoomFaceLook
    let disc: CGFloat

    var body: some View {
        let ring: FaceRing = switch look.ring {
        case .with: .with
        case .paused: .paused
        case .waiting(let part): .waiting(part)
        case .none: .none
        }
        PersonFace(name: name, ring: ring, dim: look.dim, pauseBadge: look.pauseBadge, error: look.error, disc: disc)
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
