// Where the player grows out of: the banner a play started from. The iPhone's and iPad's cover
// zooms out of it (T2); on the Mac the banner morphs into the video in the main window (M4).

import SwiftUI

extension EnvironmentValues {
    /// The namespace the player grows in from a banner, which the player's host provides.
    @Entry public var playerZoom: Namespace.ID? = nil
    /// Where the banners here stand aside, so the player does not grow out of them: the Mac's
    /// section under a title page, and the page until it has come in. Each place stands aside
    /// under its own name, so a banner standing aside never pairs with another's.
    @Entry public var playerZoomAside: String? = nil
}

extension View {
    /// This banner is where the player zooms out of when `id`, the title's, plays.
    public func playerZoomSource<ID: Hashable>(_ id: ID) -> some View {
        modifier(PlayerZoomSource(id: id))
    }
}

struct PlayerZoomSource<ID: Hashable>: ViewModifier {
    @Environment(\.playerZoom) private var zoom
    @Environment(\.playerZoomAside) private var aside
    let id: ID

    /// Standing aside keeps the banner's own identity and only changes the name it goes by.
    private struct Aside: Hashable {
        let place: String
        let id: ID
    }

    func body(content: Content) -> some View {
        if let zoom {
            let key = aside.map { AnyHashable(Aside(place: $0, id: id)) } ?? AnyHashable(id)
            #if os(macOS)
            content.matchedGeometryEffect(id: key, in: zoom, properties: .frame, isSource: true)
            #else
            content.matchedTransitionSource(id: key, in: zoom)
            #endif
        } else {
            content
        }
    }
}
