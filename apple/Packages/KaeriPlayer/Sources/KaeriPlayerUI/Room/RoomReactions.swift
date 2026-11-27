// The room's reactions: the four glyph buttons over the video, and each reaction's name.

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
        case .laugh: L.roomReactLaugh
        case .gasp: L.roomReactGasp
        case .clap: L.roomReactClap
        case .cry: L.roomReactCry
        }
    }

    static let order: [Reaction] = [.laugh, .gasp, .clap, .cry]
}

/// The room's four reaction buttons.
struct ReactionButtons: View {
    let metrics: PlayerMetrics
    var size: CGFloat = 32
    var glyph: CGFloat = 18
    var vertical = false
    @Environment(AppStore.self) private var store

    var body: some View {
        let room = store.room
        let layout = vertical ? AnyLayout(VStackLayout(spacing: 0)) : AnyLayout(HStackLayout(spacing: 0))
        layout {
            ForEach(Reaction.order, id: \.self) { reaction in
                Button { room.react(reaction) } label: {
                    ReactionGlyphView(ReactionGlyphKind(reaction), size: glyph, weight: 1.6)
                        .frame(width: size, height: size)
                        .contentShape(.circle)
                }
                .buttonStyle(OverVideoButtonStyle())
                .accessibilityLabel(reaction.buttonName)
                .help(reaction.buttonName)
            }
        }
        .accessibilityElement(children: .contain)
        .accessibilityLabel(L.roomReact)
    }
}
