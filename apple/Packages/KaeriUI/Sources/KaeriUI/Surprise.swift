// Surprise me and Another: a title picked from the screen the button is on, opened as any title
// opens, with Another on its page for that visit.

import KaeriDesign
import KaeriKit
import KaeriStore
import KaeriStrings
import SwiftUI

/// Where a pick comes from on the screen shown: the filters in Search's browse, the Planning list
/// in the Library. `nil` elsewhere, and on results for typed text.
@MainActor
func surprisePool(store: AppStore, navigator: Navigator) -> SurprisePool? {
    switch navigator.section {
    case .search:
        let query = store.search.query
        return query.text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty ? .filters(query: query) : nil
    case .library:
        return navigator.librarySegment == .planning && store.accounts.summary.state != .signedOut ? .planning : nil
    default:
        return nil
    }
}

/// The dice button: Surprise me where the screen has a pool. Past 600 ms the dice give way to a
/// spinner, and pressing again meanwhile does nothing.
struct SurpriseButton: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator

    var body: some View {
        if let pool = surprisePool(store: store, navigator: navigator) {
            Button {
                navigator.anotherAsked = false
                store.surprise(pool)
            } label: {
                SurpriseGlyph(spinning: store.search.surpriseSpinning)
            }
            .tooltip(L.surpriseMe)
            .accessibilityLabel(L.surpriseMe)
            .accessibilityHint(pool == .planning ? L.surpriseHintPlanning : L.surpriseHintFilters)
        }
    }
}

/// The dice, or a spinner in their place.
struct SurpriseGlyph: View {
    let spinning: Bool

    var body: some View {
        ZStack {
            if spinning {
                ProgressView().controlSize(.small).transition(.opacity)
            } else {
                Image(systemName: "dice").transition(.opacity)
            }
        }
        .animation(.fadeIn, value: spinning)
    }
}

/// Another, on a picked title's page for the visit: the dice and its word, in the place Surprise
/// me was. Hidden for a pool of one.
struct AnotherButton: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    let id: AnilistId

    var body: some View {
        if let visit = navigator.pickVisit, visit.id == id, visit.more {
            Button {
                navigator.anotherAsked = true
                store.surprise(visit.pool)
            } label: {
                HStack(spacing: 6) {
                    SurpriseGlyph(spinning: store.search.surpriseSpinning)
                    Text(L.surpriseAnother)
                }
            }
            .tooltip(L.surpriseAnotherHint)
            .accessibilityHint(L.surpriseAnotherHint)
        }
    }
}

extension Navigator {
    /// A pick arrived: open it, or with Another asked, swap the page in place.
    func receive(_ pick: SearchModel.Pick) {
        if anotherAsked, pickVisit != nil {
            anotherAsked = false
            replacePick(pick)
        } else {
            anotherAsked = false
            openTitle(pick.id)
            pickVisit = pick
        }
    }

    /// The page on screen becomes `pick`'s title, crossfaded in place: toolbar and sidebar do not
    /// move, and Back goes to where the picks started.
    private func replacePick(_ pick: SearchModel.Pick) {
        #if os(macOS)
        withAnimation(.standard) { titleOnMac = pick.id }
        if var path = paths[section], !path.isEmpty {
            path[path.count - 1] = TitleRoute(id: pick.id)
            paths[section] = path
        }
        #else
        if let route = paths[section]?.last {
            let root = shownInPlace.first { $0.value == route.id }?.key ?? route.id
            withAnimation(.standard) { shownInPlace[root] = pick.id }
        }
        #endif
        pickVisit = pick
    }
}

extension View {
    /// Opens the titles Surprise me picks.
    func surprisePicks() -> some View { modifier(SurprisePicks()) }
}

private struct SurprisePicks: ViewModifier {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator

    func body(content: Content) -> some View {
        content.onChange(of: store.search.pick) { _, pick in
            if let pick { navigator.receive(pick) }
        }
    }
}

/// The Library menu's Surprise Me, where the screen has a pool.
struct SurpriseMenuItem: View {
    let store: AppStore
    let navigator: Navigator

    var body: some View {
        let pool = surprisePool(store: store, navigator: navigator)
        Button(L.surpriseMeMenu) {
            if let pool {
                navigator.anotherAsked = false
                store.surprise(pool)
            }
        }
        .disabled(pool == nil)
    }
}
