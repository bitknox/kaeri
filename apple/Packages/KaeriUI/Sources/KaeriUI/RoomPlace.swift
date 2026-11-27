// Watch together in the sidebar (the room board, v5): the place to join a room and come back to
// one. Your room sits on top with its line and faces; Join a room takes an invite link; recent
// rooms rejoin. A room starts from an episode, never from here.

import KaeriDesign
import KaeriKit
import KaeriStore
import KaeriStrings
import SwiftUI
#if os(macOS)
import AppKit
#else
import UIKit
#endif

struct RoomPlace: View {
    @Environment(AppStore.self) private var store
    #if os(iOS)
    @Environment(\.horizontalSizeClass) private var sizeClass
    private var compact: Bool { sizeClass == .compact }
    #else
    private let compact = false
    #endif

    var body: some View {
        let room = store.room
        ScrollView {
            VStack(alignment: .leading, spacing: 0) {
                Text(RoomCopy.placeIntro)
                    .font(compact ? .ui(15) : .mac(14))
                    .foregroundStyle(.haze)
                    .frame(maxWidth: 640, alignment: .leading)
                    .fixedSize(horizontal: false, vertical: true)
                if room.active {
                    RoomPlaceCard(room: room, compact: compact)
                        .padding(.top, 22)
                        .transition(.opacity)
                }
                join(room).padding(.top, 26)
                recent(room).padding(.top, 26)
            }
            .padding(EdgeInsets(top: compact ? 8 : 20, leading: compact ? 16 : 32, bottom: 32, trailing: compact ? 16 : 32))
            .frame(maxWidth: .infinity, alignment: .leading)
        }
        .foregroundStyle(.silk)
        .background(.ground)
        .navigationTitle(RoomCopy.columnTitle)
        #if os(iOS)
        .navigationBarTitleDisplayMode(.large)
        #endif
        .animation(.standard, value: room.active)
        .onAppear { room.loadRecent() }
    }

    // MARK: Join a room

    private func join(_ room: RoomModel) -> some View {
        let failed = room.joinFailed != nil
        let height: CGFloat = compact ? 44 : 32
        return VStack(alignment: .leading, spacing: 8) {
            Text(RoomCopy.joinARoom).font(compact ? .ui(17, weight: .semibold) : .mac(15, weight: .semibold))
                .accessibilityAddTraits(.isHeader)
            HStack(spacing: 8) {
                HStack(spacing: 8) {
                    if room.joinText.isEmpty {
                        Image(systemName: "doc.on.doc").font(compact ? .ui(15) : .mac(13)).foregroundStyle(.haze)
                    }
                    TextField(
                    "", text: Binding(get: { room.joinText }, set: { room.joinText = $0 }),
                    prompt: Text(RoomCopy.inviteLink).foregroundStyle(.haze)
                )
                    .textFieldStyle(.plain)
                    .font(compact ? .ui(15, design: room.joinText.isEmpty ? .default : .monospaced) : .mac(13, design: room.joinText.isEmpty ? .default : .monospaced))
                    .onSubmit { room.join() }
                }
                .padding(.horizontal, 12)
                .frame(height: height)
                .background(Palette.silk.opacity(0.08), in: .rect(cornerRadius: compact ? 12 : 8))
                .overlay(RoundedRectangle(cornerRadius: compact ? 12 : 8).strokeBorder(Palette.glassEdge, lineWidth: 1))
                if failed {
                    Button(RoomCopy.tryAgain) { room.join() }
                        .buttonStyle(RoomButtonStyle(kind: .secondary, height: height, size: compact ? 15 : 13))
                } else {
                    Button(RoomCopy.paste) { paste(room) }
                        .buttonStyle(RoomButtonStyle(kind: .secondary, height: height, size: compact ? 15 : 13))
                    Button(RoomCopy.join) { room.join() }
                        .buttonStyle(RoomButtonStyle(kind: .secondary, height: height, size: compact ? 15 : 13))
                        .disabled(room.joinText.trimmingCharacters(in: .whitespaces).isEmpty)
                }
            }
            .frame(maxWidth: 640)
            if failed {
                let words = Self.words(room.joinFailure ?? .unreachable)
                failure(words.title, words.hint, size: 13)
                    .frame(maxWidth: 640, alignment: .leading)
            }
        }
    }

    private func paste(_ room: RoomModel) {
        #if os(macOS)
        room.joinText = NSPasteboard.general.string(forType: .string) ?? room.joinText
        #else
        room.joinText = UIPasteboard.general.string ?? room.joinText
        #endif
    }

    /// What a Join that didn't happen says: the title, and what to do.
    static func words(_ failure: RoomJoinFailure) -> (title: String, hint: String) {
        switch failure {
        case .unreachable: (RoomCopy.relaysUnreachableLead, RoomCopy.relaysUnreachableHint(8))
        case .full: (RoomCopy.full, "")
        case .oldInvite: (RoomCopy.oldInvite, "")
        case .failed(let reason): (reason, "")
        }
    }

    /// The failure line: a red dot, then the bold lead and the hint on one line.
    private func failure(_ title: String, _ hint: String, size: CGFloat = 12) -> some View {
        HStack(alignment: .firstTextBaseline, spacing: 8) {
            Circle().fill(Palette.vermilion).frame(width: 7, height: 7).alignmentGuide(.firstTextBaseline) { $0[.bottom] - 1 }
            // One line, as the board has it: the bold lead (with its stop), then the hint.
            Group {
                if hint.isEmpty {
                    Text(title).fontWeight(.semibold)
                } else {
                    Text("\(Text(title).fontWeight(.semibold)) \(Text(hint).foregroundStyle(.haze))")
                }
            }
            .font(compact ? .ui(13) : .mac(size))
            .fixedSize(horizontal: false, vertical: true)
        }
    }

    // MARK: Recent rooms

    private func recent(_ room: RoomModel) -> some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack(alignment: .firstTextBaseline, spacing: 10) {
                Text(RoomCopy.recentRooms).font(compact ? .ui(17, weight: .semibold) : .mac(15, weight: .semibold))
                Text(RoomCopy.lastTen).font(.board(12)).foregroundStyle(.haze)
            }
            .accessibilityElement(children: .combine)
            .accessibilityAddTraits(.isHeader)
            .padding(.bottom, 4)
            ForEach(Array(room.recent.enumerated()), id: \.element.key) { index, recent in
                VStack(alignment: .leading, spacing: 6) {
                    HStack(spacing: 12) {
                        RoomStill(id: recent.id, width: 64, height: 36, radius: 6)
                        VStack(alignment: .leading, spacing: 0) {
                            Text(recent.title).font(compact ? .ui(15, weight: .semibold) : .mac(13, weight: .semibold))
                                .fixedSize(horizontal: false, vertical: true)
                            Text(RoomCopy.withNames(recent.episode, recent.others, recent.lastAt))
                                .font(compact ? .ui(13) : .mac(12)).foregroundStyle(.haze)
                                .fixedSize(horizontal: false, vertical: true)
                            if recent.quiet, !compact {
                                failure(RoomCopy.nobodyAnsweredLead, RoomCopy.nobodyAnsweredHint).padding(.top, 4)
                            }
                        }
                        .frame(maxWidth: .infinity, alignment: .leading)
                        Button(recent.quiet ? RoomCopy.tryAgain : RoomCopy.rejoin) { room.rejoin(recent) }
                            .buttonStyle(RoomButtonStyle(kind: .secondary, height: compact ? 36 : 28, size: compact ? 15 : 12, padding: 12))
                    }
                    // On a phone Try again leaves the failure too narrow beside it: it goes under the
                    // row, from the title's edge.
                    if recent.quiet, compact {
                        failure(RoomCopy.nobodyAnsweredLead, RoomCopy.nobodyAnsweredHint).padding(.leading, 76)
                    }
                }
                .padding(.vertical, recent.quiet || compact ? 10 : 0)
                .frame(minHeight: 56)
                .frame(maxWidth: 820)
                .overlay(alignment: .top) { if index > 0 { Rectangle().fill(Palette.faintRule).frame(height: 1) } }
            }
        }
    }
}

/// Your room, on top of the place: its art, line and faces, with Open the player and Leave.
struct RoomPlaceCard: View {
    let room: RoomModel
    let compact: Bool
    @State private var asking = false

    var body: some View {
        TimelineView(.periodic(from: .now, by: 1)) { context in
            let now = RoomModel.stillAt ?? Int64((context.date.timeIntervalSince1970 * 1000).rounded())
            let status = room.status(now: now)
            let layout = compact ? AnyLayout(VStackLayout(alignment: .leading, spacing: 12)) : AnyLayout(HStackLayout(spacing: 18))
            layout {
                RoomStill(id: room.view?.episodeId ?? room.invitation?.id, width: compact ? 120 : 192, height: compact ? 68 : 108)
                VStack(alignment: .leading, spacing: 0) {
                    // An invite opened isn't your room yet: its line says who invited you.
                    if room.invitation == nil {
                        Text(RoomCopy.yourRoom).font(.board(12, weight: .semibold)).foregroundStyle(.haze)
                    }
                    Text(room.view?.title ?? room.invitation?.title ?? "").font(.showTitle(size: 18))
                        .fixedSize(horizontal: false, vertical: true)
                    Text(status.title).font(.board(15, weight: .semibold)).padding(.top, 4)
                    Text(status.detail).font(.board(13)).foregroundStyle(.haze).monospacedDigit()
                    faces(now).padding(.top, 10)
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                let stack = compact ? AnyLayout(HStackLayout(spacing: 8)) : AnyLayout(VStackLayout(spacing: 8))
                let primary = RoomButtonStyle(kind: .primary, height: compact ? 44 : 32, size: compact ? 15 : 13, padding: 16, wide: compact)
                let secondary = RoomButtonStyle(kind: .secondary, height: compact ? 44 : 32, size: compact ? 15 : 13, padding: 16, wide: compact)
                stack {
                    if case .join(let over) = status.action {
                        Button(over ? RoomCopy.joinAnyway : RoomCopy.joinTheRoom) { room.accept() }.buttonStyle(primary)
                        Button(RoomCopy.notNow) { room.notNow() }.buttonStyle(secondary)
                    } else {
                        Button(RoomCopy.openThePlayer) { room.openPlayer() }.buttonStyle(primary)
                        Button(RoomCopy.leaveDots) { asking = true }.buttonStyle(secondary)
                    }
                }
                .fixedSize(horizontal: !compact, vertical: true)
            }
        }
        .padding(16)
        .background(Palette.raised, in: .rect(cornerRadius: 16))
        .overlay(RoundedRectangle(cornerRadius: 16).strokeBorder(Palette.glassEdge, lineWidth: 1))
        .alert(RoomCopy.leaveTheRoom, isPresented: $asking) {
            Button(RoomCopy.cancel, role: .cancel) {}
            Button(RoomCopy.leave, role: .destructive) { room.leave() }
        } message: {
            Text(RoomCopy.othersKeepWatching)
        }
    }

    private func faces(_ now: Int64) -> some View {
        let people = room.people
        // A row when it fits; otherwise one face to a line, so every word stays whole.
        return ViewThatFits(in: .horizontal) {
            HStack(spacing: 12) { ForEach(people) { face($0, now) } }
            VStack(alignment: .leading, spacing: 6) { ForEach(people) { face($0, now) } }
        }
        .accessibilityElement(children: .combine)
    }

    private func face(_ member: RoomMember, _ now: Int64) -> some View {
        let look = room.look(member, now: now)
        return HStack(spacing: 6) {
            PersonFace(name: member.name, ring: ring(member, now), dim: look.dim, pauseBadge: look.pauseBadge, error: look.error, disc: 22)
            Text("\(member.you ? RoomCopy.you : member.name) \(Text(room.word(member, now: now)).foregroundStyle(.haze))")
                .font(.board(12))
                .lineLimit(1)
        }
    }

    private func ring(_ member: RoomMember, _ now: Int64) -> FaceRing {
        switch room.look(member, now: now).ring {
        case .with: .with
        case .paused: .paused
        case .waiting(let part): .waiting(part)
        case .none: .none
        }
    }
}

/// The room's still: the title's wide art, or its cover's colour. On iPhone, and an iPad at
/// compact width, by the wide art's rule (`HeroArt`): its banner, a seen still, the cover filling
/// the frame, or a plate with the title.
struct RoomStill: View {
    @Environment(AppStore.self) private var store
    @Environment(\.displayScale) private var scale
    #if os(iOS)
    @Environment(\.horizontalSizeClass) private var sizeClass
    #endif
    let id: AnilistId?
    let width: CGFloat
    let height: CGFloat
    var radius: CGFloat = 10

    /// Whether the frame fills by the wide art's rule.
    private var fills: Bool {
        #if os(iOS)
        sizeClass == .compact
        #else
        false
        #endif
    }

    var body: some View {
        let entry = id.flatMap { store.titles[$0] }
        Group {
            if fills, let entry {
                HeroArtView(art: store.heroArt(entry, fills: true), entry: entry)
            } else {
                ArtworkView(url: entry?.bannerUrl ?? entry?.coverUrl, color: entry?.plateColor,
                            maxPixelSize: Int(width * scale))
            }
        }
            .frame(width: width, height: height)
            .clipShape(.rect(cornerRadius: radius))
            .accessibilityHidden(true)
            .task(id: id) {
                // The room's title may be one this device has not opened: its art and episodes.
                guard let id else { return }
                if store.titles[id] == nil { store.loadDetail(id) }
                if store.titles.episodes[id] == nil { store.loadEpisodes(id) }
            }
    }
}
