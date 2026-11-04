// Chat over the video: the column at the top leading corner (the engine's notice first, chat lines
// under it, three rows at most, the rest folded into "n more in chat"), the reaction moment at the
// bottom trailing corner, the glyph buttons, and the ticks on the seek line.

import KaeriDesign
import KaeriKit
import KaeriStore
import KaeriStrings
import SwiftUI

extension ReactionGlyphKind {
    init(_ reaction: Reaction) {
        switch reaction {
        case .laugh: self = .laugh
        case .gasp: self = .gasp
        case .clap: self = .clap
        case .cry: self = .cry
        }
    }
}

extension Reaction {
    /// "Laugh", the button's name.
    var buttonName: String {
        switch self {
        case .laugh: L.partyReactLaugh
        case .gasp: L.partyReactGasp
        case .clap: L.partyReactClap
        case .cry: L.partyReactCry
        }
    }

    static let order: [Reaction] = [.laugh, .gasp, .clap, .cry]
}

extension ChatLineData {
    init(_ row: ChatRow) {
        switch row.kind {
        case .said(let from, let text, let you):
            self.init(id: row.id, name: you ? L.partyChatYou : from, text: text, you: you)
        case .reacted(let text):
            self.init(id: row.id, name: nil, text: text)
        }
    }
}

/// What the notice column holds: the engine's notice, then the chat's lines. A line that leaves
/// fades out where it stood, and 0.3 s later the column closes up under it on `standard`; the first
/// line past the room is drawn as the fold, so when it comes into view it crossfades in the fold's
/// place. With Reduce Motion nothing moves: the column crossfades from the old places to the new.
struct PartyColumn: View {
    let model: PlayerModel
    let metrics: PlayerMetrics
    /// The people popover or sheet is open, and shows the same lines.
    let covered: Bool
    /// "n more in chat" was pressed: the conversation opens.
    /// The rows it may take, notices included: fewer while the phone's field is up under it.
    var rows = PartyChat.columnRows
    let openChat: () -> Void
    @Environment(AppStore.self) private var store
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    /// The lines drawn, oldest first: the column's, and the ones still fading out where they stood.
    @State private var drawn: [ChatColumnLine] = []
    @State private var leaving: Set<UInt64> = []
    /// Counts the column's close-ups, so Reduce Motion can crossfade the whole column on each.
    @State private var closes = 0

    private var width: CGFloat { metrics.touch ? 340 : 420 }

    /// How long a leaving line fades before the column closes up under it.
    static let closeDelay = Duration.milliseconds(300)

    var body: some View {
        let chat = store.party.chat
        let live = store.party.inParty && chat.shown && !covered
        let staying = drawn.filter { !leaving.contains($0.id) }
        let folded = max(0, staying.count - room)
        VStack(alignment: .leading, spacing: 6) {
            if let alert = model.alert {
                notice(alert)
                    .transition(transition)
            }
            ZStack(alignment: .topLeading) {
                VStack(alignment: .leading, spacing: 6) {
                    ForEach(drawn) { line in
                        let rank = staying.firstIndex { $0.id == line.id }
                        // No fold while the phone's field is up: its bar takes that room.
                        if room > 0, rank.map({ $0 < room || ($0 == room && rows == PartyChat.columnRows) }) ?? true {
                            ZStack(alignment: .leading) {
                                if let rank, rank == room {
                                    fold(folded).transition(swap)
                                } else {
                                    row(line)
                                        .opacity(rank == nil ? 0 : 1)
                                        .accessibilityHidden(rank == nil)
                                        .transition(swap)
                                }
                            }
                            .transition(transition)
                        }
                    }
                }
                .id(reduceMotion ? closes : 0)
                .transition(.asymmetric(insertion: .opacity.animation(.fadeIn), removal: .opacity.animation(.fadeOut)))
            }
        }
        .frame(maxWidth: width, alignment: .leading)
        .animation(reduceMotion ? .fadeIn : .standard, value: model.alert)
        .onChange(of: live ? chat.column.map(\.id) : [], initial: true) { old, ids in
            follow(live ? chat.column : [], ids: ids, animated: old != ids)
        }
        .task(id: Signature(ids: chat.column.map(\.id), room: room, live: live)) {
            guard live else { return }
            while !Task.isCancelled {
                guard let next = chat.settle(room: room) else { return }
                try? await Task.sleep(until: next, clock: .continuous)
            }
        }
    }

    /// Take the column's new lines in, and fade out the ones it let go, closing up after.
    private func follow(_ column: [ChatColumnLine], ids: [UInt64], animated: Bool) {
        let now = Set(ids)
        let inView = Set(drawn.filter { !leaving.contains($0.id) }.prefix(room).map(\.id))
        let letGo = drawn.filter { !now.contains($0.id) && !leaving.contains($0.id) }.map(\.id)
        // A line in view fades where it stands; one that never came into view just goes.
        let gone = letGo.filter(inView.contains)
        var next = drawn.filter { now.contains($0.id) || leaving.contains($0.id) || gone.contains($0.id) }
        let known = Set(next.map(\.id))
        next.append(contentsOf: column.filter { !known.contains($0.id) })
        var arriving = Transaction(animation: animated ? (reduceMotion ? .fadeIn : .standard) : nil)
        arriving.disablesAnimations = !animated
        withTransaction(arriving) { drawn = next }
        guard !gone.isEmpty else { return }
        // The line fades where it stands, and the folded line crossfades into the fold's place.
        withAnimation(.fadeOut) { leaving.formUnion(gone) }
        Task { @MainActor in
            try? await Task.sleep(for: Self.closeDelay)
            withAnimation(reduceMotion ? .fadeIn : .standard) {
                drawn.removeAll { gone.contains($0.id) }
                leaving.subtract(gone)
                closes += 1
            }
        }
    }

    /// The rows left for chat lines under the notice.
    private var room: Int { max(0, rows - (model.alert == nil ? 0 : 1)) }

    private struct Signature: Equatable {
        var ids: [UInt64]
        var room: Int
        var live: Bool
    }

    private var height: CGFloat { metrics.touch ? 36 : 32 }

    /// The fold and the line it held trade places: the fade-out and the fade-in together.
    private var swap: AnyTransition {
        .asymmetric(insertion: .opacity.animation(.fadeIn), removal: .opacity.animation(.fadeOut))
    }

    private var transition: AnyTransition {
        reduceMotion
            ? .opacity
            : .asymmetric(insertion: .opacity.combined(with: .offset(y: 6)), removal: .opacity)
    }

    private func fold(_ count: Int) -> some View {
        Button(action: openChat) {
            Text(L.partyChatMore(count: max(count, 1)))
                .font(.ui(metrics.body))
                .foregroundStyle(Palette.haze)
                .padding(.horizontal, 14)
                .frame(minHeight: height)
                .overVideo(radius: height / 2)
                .contentShape(.capsule)
        }
        .buttonStyle(.plain)
    }

    private func notice(_ alert: PlayerAlert) -> some View {
        Label(alert.text, systemImage: alert.kind == .alert ? "exclamationmark.triangle.fill" : "info.circle")
            .font(.system(size: metrics.body))
            .foregroundStyle(alert.kind == .alert ? Palette.vermilion : Palette.silk)
            .lineLimit(3)
            .padding(.horizontal, 14)
            .frame(minHeight: height)
            .overVideo(radius: height / 2)
            .fixedSize(horizontal: false, vertical: true)
    }

    private func row(_ line: ChatColumnLine) -> some View {
        (Text(line.you ? L.partyChatYou : line.from).fontWeight(.semibold) + Text("  ") + Text(line.text))
            .font(.ui(metrics.body))
            .foregroundStyle(Palette.silk)
            .lineLimit(3)
            .padding(.horizontal, 14)
            .frame(minHeight: height)
            .overVideo(radius: height / 2)
            .fixedSize(horizontal: false, vertical: true)
            .accessibilityElement(children: .ignore)
            .accessibilityLabel(L.partyChatAnnounce(name: line.you ? L.partyChatYou : line.from, message: line.text))
    }
}

/// A reaction as it happens: its glyph and who made it, for 1.5 s at the bottom trailing corner.
struct ReactionMomentView: View {
    let metrics: PlayerMetrics
    @Environment(AppStore.self) private var store
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var shown: ReactionMoment?

    var body: some View {
        let chat = store.party.chat
        ZStack {
            if let moment = shown {
                HStack(spacing: 8) {
                    ReactionGlyphView(ReactionGlyphKind(moment.reaction), size: metrics.touch ? 20 : 18, weight: 1.6)
                    Text(moment.you ? L.partyChatYou : moment.from)
                        .font(.ui(metrics.body, weight: .semibold))
                }
                .foregroundStyle(Palette.silk)
                .padding(.horizontal, 12)
                .frame(minHeight: metrics.touch ? 36 : 32)
                .overVideo(radius: metrics.touch ? 18 : 16)
                .id(moment.id)
                .transition(
                    reduceMotion
                        ? .opacity
                        : .asymmetric(insertion: .opacity.combined(with: .offset(y: 6)), removal: .opacity)
                )
                .accessibilityElement(children: .ignore)
                .accessibilityLabel(PartyChat.reacted(moment.reaction, moment.you ? L.partyChatYou : moment.from))
            }
        }
        .animation(reduceMotion ? .fadeIn : .standard, value: shown)
        .task(id: chat.moment?.id) {
            guard let moment = chat.moment else { return }
            shown = moment
            try? await Task.sleep(for: PartyChat.momentStays)
            if Task.isCancelled { return }
            shown = nil
            chat.clearMoment(moment.id)
        }
    }
}

/// The four reaction buttons. They sit at 40% while a reaction's wait runs.
struct ReactionButtons: View {
    let metrics: PlayerMetrics
    var size: CGFloat = 32
    var glyph: CGFloat = 18
    var vertical = false
    @Environment(AppStore.self) private var store

    var body: some View {
        let chat = store.party.chat
        let waiting = chat.reactWait > 0
        let layout = vertical ? AnyLayout(VStackLayout(spacing: 0)) : AnyLayout(HStackLayout(spacing: 0))
        layout {
            ForEach(Reaction.order, id: \.self) { reaction in
                Button { chat.react(reaction) } label: {
                    ReactionGlyphView(ReactionGlyphKind(reaction), size: glyph, weight: 1.6)
                        .frame(width: size, height: size)
                        .contentShape(.circle)
                }
                .buttonStyle(OverVideoButtonStyle())
                .accessibilityLabel(reaction.buttonName)
                .help(reaction.buttonName)
                .disabled(waiting)
            }
        }
        .opacity(waiting ? 0.4 : 1)
        .animation(.fadeIn, value: waiting)
        .accessibilityElement(children: .contain)
        .accessibilityLabel(L.partyReact)
    }
}

/// The room's reaction ticks on the seek line: 2 by 5 pt in haze, 3 pt above the line.
struct ReactionTicks: View {
    let ticks: [ReactionTick]
    let lineHeight: CGFloat

    var body: some View {
        ZStack(alignment: .leading) {
            ForEach(ticks.indices, id: \.self) { index in
                Capsule()
                    .fill(Palette.haze)
                    .frame(width: 2, height: 5)
                    .offset(x: ticks[index].x - 1, y: -(lineHeight / 2 + 3 + 2.5))
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .leading)
        .transition(.opacity)
        .allowsHitTesting(false)
        .accessibilityHidden(true)
    }
}
