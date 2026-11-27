// The room folded into the bar over the tabs on iPhone (the room board, v5): the episode's art,
// the show and the room's line, the faces, and play or pause. A tap opens the player again at the
// room's place. Inline, as the tab bar minimises, the show alone.

#if os(iOS)
import KaeriDesign
import KaeriKit
import KaeriStore
import KaeriStrings
import SwiftUI

public struct PhoneRoomBar: View {
    @Environment(AppStore.self) private var store
    @Environment(\.tabViewBottomAccessoryPlacement) private var placement

    public init() {}

    public var body: some View {
        let room = store.room
        if let view = room.view {
            RoomClock { now in
                let status = room.status(now: now)
                let waits = !view.waiting.isEmpty
                HStack(spacing: 10) {
                    if placement != .inline {
                        RoomBanner(id: view.episodeId)
                            .frame(width: 56, height: 44)
                            .clipShape(.capsule)
                    }
                    VStack(alignment: .leading, spacing: 0) {
                        Text(view.title).font(.showTitle(size: 14)).foregroundStyle(.silk).lineLimit(1)
                        if placement != .inline {
                            Text(status.title)
                                .font(.board(13))
                                .foregroundStyle(waits ? .silk : .haze)
                                .monospacedDigit()
                                .lineLimit(1)
                                .contentTransition(.opacity)
                                .animation(.fadeIn, value: status.title)
                        }
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)
                    if placement != .inline {
                        HStack(spacing: -6) {
                            ForEach(RoomFaces.capped(room.people).shown) { member in
                                RoomFace(name: member.name, look: room.look(member, now: now), disc: 22)
                            }
                        }
                        .accessibilityHidden(true)
                    }
                    let playing = view.playing && !waits
                    Button { room.setPlaying(!playing) } label: {
                        Image(systemName: playing ? "pause.fill" : "play.fill")
                            .font(.board(20, weight: .semibold))
                            .foregroundStyle(.silk)
                            .frame(width: 44, height: 44)
                            .contentShape(.rect)
                    }
                    .buttonStyle(.plain)
                    .accessibilityLabel(playing ? L.actionPause : L.actionPlay)
                }
                .padding(.leading, placement == .inline ? 16 : 8)
                .padding(.trailing, 8)
                .contentShape(.rect)
                .onTapGesture { room.openPlayer() }
                .accessibilityElement(children: .contain)
                .accessibilityAction(named: RoomCopy.openThePlayer) { room.openPlayer() }
            }
        }
    }
}
#endif
