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

/// What the notice column holds: the engine's notice, then the chat's lines.
struct PartyColumn: View {
    let model: PlayerModel
    let metrics: PlayerMetrics
    /// The people popover or sheet is open, and shows the same lines.
    let covered: Bool
    /// "n more in chat" was pressed: the conversation opens.
    let openChat: () -> Void
    @Environment(AppStore.self) private var store
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    private var width: CGFloat { metrics.touch ? 340 : 420 }

    var body: some View {
        let chat = store.party.chat
        let live = store.party.inParty && chat.shown && !covered
        let noticeRows = model.alert == nil ? 0 : 1
        let room = max(0, PartyChat.columnRows - noticeRows)
        let lines = live ? chat.column.filter { $0.shown != nil }.prefix(room) : []
        let folded = live ? chat.folded(room: room) : 0
        VStack(alignment: .leading, spacing: 6) {
            if let alert = model.alert {
                notice(alert)
                    .transition(transition)
            }
            ForEach(Array(lines)) { line in
                row(line)
                    .transition(transition)
            }
            if folded > 0 {
                Button(action: openChat) {
                    Text(L.partyChatMore(count: folded))
                        .font(.system(size: metrics.body))
                        .foregroundStyle(Palette.haze)
                        .padding(.horizontal, 14)
                        .frame(height: height)
                        .overVideo(radius: height / 2)
                        .contentShape(.capsule)
                }
                .buttonStyle(.plain)
                .transition(transition)
            }
        }
        .frame(maxWidth: width, alignment: .leading)
        .animation(reduceMotion ? .fadeIn : .standard, value: model.alert)
        .animation(reduceMotion ? .fadeIn : .standard, value: lines.map(\.id))
        .animation(reduceMotion ? .fadeIn : .standard, value: folded)
        .task(id: Signature(ids: chat.column.map(\.id), room: room, live: live)) {
            guard live else { return }
            while !Task.isCancelled {
                guard let next = chat.settle(room: room) else { return }
                try? await Task.sleep(until: next, clock: .continuous)
            }
        }
    }

    private struct Signature: Equatable {
        var ids: [UInt64]
        var room: Int
        var live: Bool
    }

    private var height: CGFloat { metrics.touch ? 36 : 32 }

    private var transition: AnyTransition {
        reduceMotion
            ? .opacity
            : .asymmetric(insertion: .opacity.combined(with: .offset(y: 6)), removal: .opacity)
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
            .font(.system(size: metrics.body))
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
                        .font(.system(size: metrics.body, weight: .semibold))
                }
                .foregroundStyle(Palette.silk)
                .padding(.horizontal, 12)
                .frame(height: metrics.touch ? 36 : 32)
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
