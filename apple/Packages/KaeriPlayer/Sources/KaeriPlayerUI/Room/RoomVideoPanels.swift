// What the video area shows when your own player has nothing to show, or before play: the
// episode's banner with the room's words over it (the room board, v5). The room carries on in the
// column beside it.

import KaeriDesign
import KaeriKit
import KaeriStore
import SwiftUI

/// The room's episode as a full-bleed banner, by the hero-art rule.
struct RoomBanner: View {
    let id: AnilistId?
    @Environment(AppStore.self) private var store

    var body: some View {
        let entry = id.flatMap { store.titles[$0] }
        let url = entry.map { store.heroArt($0).url } ?? nil
        let image = url.flatMap { store.artwork.image($0) }
        ZStack {
            Palette.ground
            if let image {
                FilledArt(image: image).transition(.opacity)
            }
        }
        .animation(.fadeIn, value: image != nil)
        .clipped()
        .task(id: url) {
            if let url { await store.artwork.show(url, maxPixelSize: ScreenPixels.longSide) }
        }
    }
}

/// Which panel covers the video.
enum RoomVideoPanel: Equatable {
    /// Before play, or opened from an invite and not joined yet.
    case beforePlay(invited: Bool)
    case choosing
    case noSource
}

extension RoomModel {
    /// The panel over the video, when one covers it.
    var videoPanel: RoomVideoPanel? {
        if invitation != nil { return .beforePlay(invited: true) }
        guard let view else { return nil }
        switch you?.state {
        case .choosing: return .choosing
        case .noSource: return .noSource
        default: break
        }
        if !view.started { return .beforePlay(invited: false) }
        return nil
    }

    var episodeId: AnilistId? { invitation?.episode.id ?? view?.episode.id }
}

/// The Mac's panel: the banner under a dusk gradient, the words at the bottom leading corner.
struct RoomMacVideoPanel: View {
    let room: RoomModel
    let panel: RoomVideoPanel

    var body: some View {
        GeometryReader { proxy in
            ZStack(alignment: .topLeading) {
                RoomBanner(id: room.episodeId)
                LinearGradient(
                    stops: [
                        .init(color: Color(rgb: 0x0B0D18).opacity(0.92), location: 0),
                        .init(color: Color(rgb: 0x0B0D18).opacity(0.55), location: 0.6),
                        .init(color: Color(rgb: 0x0B0D18).opacity(0.35), location: 1),
                    ], startPoint: .bottom, endPoint: .top)
                content
                    .padding(.leading, 56)
                    .padding(.top, max(60, proxy.size.height - (panel == .choosing ? 330 : 236)))
                    .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
            }
        }
        .foregroundStyle(.silk)
    }

    private var episode: RoomEpisode? { room.invitation?.episode ?? room.view?.episode }
    private var title: String { room.invitation?.title ?? room.view?.title ?? "" }

    @ViewBuilder
    private var content: some View {
        switch panel {
        case .beforePlay(let invited):
            VStack(alignment: .leading, spacing: 0) {
                Text(invited ? RoomCopy.roomIsWatching : RoomCopy.upInTheRoom)
                    .font(.system(size: 13, weight: .semibold)).foregroundStyle(.haze)
                Text(title).font(.macShowTitle(size: 40)).lineLimit(1).padding(.top, 6)
                Text(episodeLine).font(.system(size: 17)).padding(.top, 4)
                HStack(spacing: 8) {
                    Circle().fill(Palette.water).frame(width: 7, height: 7)
                    Text(RoomCopy.playsFromYourSources)
                }
                .font(.system(size: 14)).foregroundStyle(.haze).padding(.top, 10)
                if !invited, room.view?.waiting.isEmpty == true, room.view?.startingIn == nil {
                    HStack(spacing: 12) {
                        Button { room.play() } label: {
                            Label(RoomCopy.playForEveryone, systemImage: "play.fill")
                        }
                        .buttonStyle(RoomButtonStyle(kind: .glass, height: 44, size: 15, padding: 22))
                        Text(RoomCopy.anyoneCanStart).font(.system(size: 13)).foregroundStyle(.haze)
                    }
                    .padding(.top, 22)
                }
            }
            .frame(maxWidth: 640, alignment: .leading)
        case .choosing:
            VStack(alignment: .leading, spacing: 0) {
                Text(title).font(.macShowTitle(size: 28)).lineLimit(1)
                Text(RoomCopy.chooseSourceFor(episode?.episode ?? "")).font(.system(size: 15)).padding(.top, 2)
                Text(RoomCopy.roomKeepsWatching).font(.system(size: 13)).foregroundStyle(.haze).padding(.top, 4)
                RoomSourceList(room: room, rowHeight: 52, nameSize: 14)
                    .padding(.top, 14)
                HStack(spacing: 10) {
                    Button(RoomCopy.playInTheRoom) { room.playChosen() }
                        .buttonStyle(RoomButtonStyle(kind: .primary, height: 36, size: 14, padding: 18))
                        .disabled(room.chosen == nil)
                    Button(RoomCopy.notNow) { room.stopChoosing() }
                        .buttonStyle(RoomButtonStyle(kind: .glass, height: 36, size: 14, padding: 16))
                }
                .padding(.top, 14)
            }
            .frame(width: 520, alignment: .leading)
        case .noSource:
            VStack(alignment: .leading, spacing: 0) {
                Text(title).font(.macShowTitle(size: 28)).lineLimit(1)
                Text(RoomCopy.noneHas(episode?.episode ?? "")).font(.system(size: 17, weight: .semibold)).padding(.top, 6)
                Text(RoomCopy.lookedIn(room.sourceCount)).font(.system(size: 14)).foregroundStyle(.haze)
                    .fixedSize(horizontal: false, vertical: true).padding(.top, 4)
                HStack(spacing: 10) {
                    Button(RoomCopy.openSources) { room.openSources() }
                        .buttonStyle(RoomButtonStyle(kind: .primary, height: 36, size: 14, padding: 18))
                    Button { room.searchTitles() } label: { Label(RoomCopy.searchTitles, systemImage: "magnifyingglass") }
                        .buttonStyle(RoomButtonStyle(kind: .glass, height: 36, size: 14, padding: 16))
                }
                .padding(.top, 16)
            }
            .frame(width: 560, alignment: .leading)
        }
    }

    private var episodeLine: String {
        guard let episode else { return "" }
        var parts = [room.episodeName(episode.episode)]
        if let name = episode.name { parts[0] += " · \(name)" }
        if let duration = episode.duration { parts.append(RoomCopy.minutes(Int((duration / 60).rounded()))) }
        return parts.joined(separator: " · ")
    }
}

/// The sources that list the room's episode, one picked.
struct RoomSourceList: View {
    let room: RoomModel
    var rowHeight: CGFloat = 52
    var nameSize: CGFloat = 14

    var body: some View {
        VStack(spacing: 0) {
            ForEach(Array(room.choices.enumerated()), id: \.element.id) { index, choice in
                Button { room.chosen = choice.id } label: {
                    HStack(spacing: 12) {
                        VStack(alignment: .leading, spacing: 0) {
                            Text(choice.name).font(.system(size: nameSize, weight: .semibold))
                            Text(choice.detail).font(.system(size: 12)).foregroundStyle(.haze)
                        }
                        .frame(maxWidth: .infinity, alignment: .leading)
                        if room.chosen == choice.id {
                            Image(systemName: "checkmark").font(.system(size: 15, weight: .semibold))
                        }
                    }
                    .padding(.horizontal, 14)
                    .frame(height: rowHeight)
                    .background(room.chosen == choice.id ? Palette.silk.opacity(0.08) : .clear)
                    .contentShape(.rect)
                }
                .buttonStyle(.plain)
                .overlay(alignment: .top) { if index > 0 { Rectangle().fill(Palette.glassEdge).frame(height: 1) } }
                .accessibilityAddTraits(room.chosen == choice.id ? .isSelected : [])
            }
        }
        .background(.ultraThinMaterial, in: .rect(cornerRadius: 14))
        .background(Palette.glass.opacity(0.6), in: .rect(cornerRadius: 14))
        .overlay(RoundedRectangle(cornerRadius: 14).strokeBorder(Palette.glassOverArtEdge, lineWidth: 1))
        .clipShape(.rect(cornerRadius: 14))
    }
}

/// Your own player opening while the room waits for it: the starting screen says so, and Don't
/// wait for me lets the room go on at once.
struct RoomDontWaitNote: View {
    let room: RoomModel

    var body: some View {
        RoomClock { now in
            let since = room.view?.waiting.first(where: { $0.name == room.you?.name })?.sinceMs ?? now
            VStack(spacing: 16) {
                Text(RoomCopy.seconds(RoomCopy.roomPausedUntil, RoomModel.seconds(since: since, now: now)))
                    .font(.system(size: 13)).foregroundStyle(.haze).monospacedDigit()
                Button(RoomCopy.dontWait) { room.dontWait() }
                    .buttonStyle(RoomButtonStyle(kind: .glass, height: 32, size: 13, padding: 16))
            }
        }
    }
}
