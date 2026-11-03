// Transitions started by a launch argument, for frame sequences of the motion audit: the Fixtures
// app and the dev harness run one once the store has settled, and the shots record it.

import KaeriKit
import KaeriStore
import SwiftUI

/// A transition from motion.md the app can be asked to run, with `-KaeriMotion <name>`.
public enum MotionDemo: String, CaseIterable, Sendable {
    /// T1: a cover zooms into its title's page (iPhone and iPad): the first Continue card on Home.
    case t1
    /// T1 from the first poster on Home's first shelf of posters: into the page's cover on the Mac.
    case t1Poster
    /// T1 from Home's spotlight, whose title is often on a shelf too: out of the spotlight.
    case t1Spotlight
    /// T1 from the first cover of the grid on screen: the Library's list, or Seasonal.
    case t1Grid
    /// M1 to a section `-KaeriTo` names, as a click in the sidebar does: its first load.
    case section
    /// Home asked for again as on its first visit, with nothing kept: its placeholders, then its
    /// sections arriving.
    case homeFirst
    /// M2: Continue's selection moves to the next title, the preview crossfading.
    case m2
    /// M3: the preview's banner grows into the title page.
    case m3
    /// M3 in reverse: Back from a title page opened from the preview.
    case back
    /// Back from the page of Home's spotlight, opened from the spotlight first (`prelude`): its
    /// art flies home.
    case backSpotlight
    /// T9: the title's next episode is marked watched.
    case t9
    /// The title page's next season opened from its season picker, as a click on its cell does.
    case season
    /// Search: the words `-KaeriType` gives typed into the field, a key every 120 ms.
    case type
    /// Search: the filters panel opened, or closed when it is open.
    case filters
    /// Search: TV toggled in the format filter, as a click in its menu does.
    case refine
    /// Search: the filters changed three times, 1.2 s apart, as clicks in their menus and on a
    /// chip do: TV added, then TV changed to Movie, then the genres taken off.
    case pills
    /// Search: Clear all, as a tap on the pinned row's first pill does.
    case clear
    /// Search: the iPhone's pinned row scrolled sideways to its end.
    case rowEnd
    /// The Mac's hover card: a pointer path rests on a cover, hands the card to the next cover
    /// and leaves it (the hover card board's Motion studies).
    case hover
    /// Home: the spotlight's episode marked watched, the spotlight moving on (Home board,
    /// prototype 2).
    case homeWatched
    /// Home: Not interested on the first poster that is not Continue's or the week's (Home
    /// board, prototype 6).
    case homeHide
    /// The title `-KaeriOpen` names opened from nowhere, as the palette does: its page coming in,
    /// its episode rows laid out.
    case titleOpen
    /// The title page's episodes scrolled to the last row of their range, then the next range
    /// picked, as View › Next Episodes does: the rows crossfading, the pickers coming back.
    case episodeRange
    /// Show on the line where spoiler protection starts, then Hide 1.6 s later.
    case protectionLine
    /// Show synopsis on the first row spoiler protection hides, as its menu does.
    case reveal

    /// The one the launch arguments name.
    public static func from(arguments: [String] = ProcessInfo.processInfo.arguments) -> MotionDemo? {
        guard let index = arguments.firstIndex(of: "-KaeriMotion"), arguments.indices.contains(index + 1)
        else { return nil }
        return MotionDemo(rawValue: arguments[index + 1])
    }

    /// What has to happen before it runs, and settle: the page opened from where it goes back to,
    /// or the page whose seasons it picks from, held back as long as the harness says.
    @MainActor
    public func prelude(navigator: Navigator, store: AppStore) async {
        switch self {
        case .backSpotlight:
            MotionDemo.t1Spotlight.run(navigator: navigator, store: store)
            try? await Task.sleep(for: .seconds(2))
        case .episodeRange, .protectionLine, .reveal:
            for _ in 0..<200 {
                if let id = navigator.titleOnMac ?? navigator.paths[navigator.section]?.last?.id,
                   store.titles.episodes[id] != nil { break }
                try? await Task.sleep(for: .milliseconds(50))
            }
            if self == .episodeRange {
                try? await Task.sleep(for: .milliseconds(600))
                navigator.episodeListToEnd += 1
                try? await Task.sleep(for: .milliseconds(1200))
            }
        case .season:
            for _ in 0..<200 {
                if let id = navigator.titleOnMac ?? navigator.paths[navigator.section]?.last?.id,
                   store.titles[id]?.detail?.seasons != nil, store.titles.episodes[id] != nil { break }
                try? await Task.sleep(for: .milliseconds(50))
            }
        default:
            break
        }
    }

    /// Run it on the screen the launch opened.
    @MainActor
    public func run(navigator: Navigator, store: AppStore) {
        let queue = store.continuing.queue(store.titles).all.map(\.id)
        switch self {
        case .t1:
            guard let id = queue.first else { return }
            let entry = store.titles[id]
            let wide = entry.map { !store.heroArt($0).isCompact } ?? false
            navigator.openTitle(id, from: .shelf(id, .continue, art: wide ? .banner : .poster))
        case .t1Poster:
            let shelves = store.discovery.view?.sections.filter { $0.kind != .continue && $0.kind != .thisWeek } ?? []
            guard let shelf = shelves.first(where: { !$0.items.isEmpty }), let item = shelf.items.first else { return }
            navigator.openTitle(item.entry.id, from: .shelf(item.entry.id, shelf.kind, art: .poster))
        case .t1Spotlight:
            guard let spot = store.discovery.view?.spotlight else { return }
            let entry = store.titles[spot.entry.id] ?? spot.entry
            navigator.openTitle(entry.id, from: TitleSource(entry.id, .spotlight, art: store.heroArt(entry).isCompact ? .poster : .banner))
        case .t1Grid:
            let ids = navigator.section == .seasonal
                ? store.seasonal.state.ids
                : store.library.segment(navigator.librarySegment).state.ids
            guard let id = ids.first else { return }
            navigator.openTitle(id, from: TitleSource(id, .grid, art: .poster))
        case .section:
            let arguments = ProcessInfo.processInfo.arguments
            guard let at = arguments.firstIndex(of: "-KaeriTo"), arguments.indices.contains(at + 1),
                  let section = AppSection(rawValue: arguments[at + 1])
            else { return }
            var instant = Transaction()
            instant.disablesAnimations = true
            withTransaction(instant) { navigator.section = section }
        case .homeFirst:
            store.discovery.forgetForDemo()
            store.loadHome(force: true)
        case .m2:
            if queue.count > 1 { navigator.continueSelection = queue[1] }
        case .m3:
            if let id = navigator.focusedTitle ?? queue.first { navigator.openTitle(id) }
        case .back, .backSpotlight:
            navigator.back()
        case .t9:
            guard let id = navigator.focusedTitle, let entry = store.titles[id] else { return }
            let next = entry.progress?.next ?? 1
            store.send(.setWatched(id: id, episodes: [String(next)], watched: true))
        case .season:
            guard let id = navigator.titleOnMac ?? navigator.paths[navigator.section]?.last?.id,
                  let seasons = store.titles[id]?.detail?.seasons,
                  let at = seasons.firstIndex(where: { $0.id == id }), seasons.indices.contains(at + 1)
            else { return }
            navigator.openTitle(seasons[at + 1].id)
        case .type:
            let arguments = ProcessInfo.processInfo.arguments
            guard let at = arguments.firstIndex(of: "-KaeriType"), arguments.indices.contains(at + 1)
            else { return }
            let words = arguments[at + 1]
            Task { @MainActor in
                for end in words.indices {
                    navigator.query = String(words[...end])
                    try? await Task.sleep(for: .milliseconds(120))
                }
            }
        case .filters:
            navigator.searchFiltersShown.toggle()
        case .refine:
            store.searchLive(store.search.query.togglingFormat(format: .tv))
        case .pills:
            Task { @MainActor in
                store.searchLive(store.search.query.togglingFormat(format: .tv))
                try? await Task.sleep(for: .milliseconds(1200))
                store.searchLive(store.search.query.togglingFormat(format: .tv).togglingFormat(format: .movie))
                try? await Task.sleep(for: .milliseconds(1200))
                store.editSearch { $0.genres = []; $0.genresExcluded = [] }
            }
        case .clear:
            store.searchLive(store.search.query.cleared())
        case .rowEnd:
            navigator.searchRowToEnd += 1
        case .hover:
            #if os(macOS)
            HoverCardScript.requested = true
            #endif
        case .homeWatched:
            guard let spot = store.discovery.view?.spotlight else { return }
            store.send(.setWatched(id: spot.entry.id, episodes: [String(spot.episode)], watched: true))
            navigator.watchedByHand += 1
        case .homeHide:
            let shelves = store.discovery.view?.sections.filter { $0.kind != .continue && $0.kind != .thisWeek } ?? []
            guard let item = shelves.first?.items.first else { return }
            store.notInterested(item.entry, title: item.entry.showTitle, unaired: item.starts != nil)
        case .titleOpen:
            let arguments = ProcessInfo.processInfo.arguments
            guard let at = arguments.firstIndex(of: "-KaeriOpen"), arguments.indices.contains(at + 1),
                  let id = AnilistId(arguments[at + 1])
            else { return }
            navigator.openTitle(id)
        case .episodeRange:
            if navigator.episodeRangesShown != nil {
                navigator.stepEpisodeRange(by: 1)
            } else if let id = navigator.paths[navigator.section]?.last?.id,
                      let rows = store.titles.episodes[id] {
                // A phone has no View menu: the next range is picked as its picker picks it.
                let plan = EpisodePlan(rows: rows, picked: navigator.episodeRange[id])
                guard plan.index + 1 < plan.ranges.count else { return }
                navigator.episodeRange[id] = plan.index + 1
            }
        case .protectionLine:
            guard let id = navigator.titleOnMac ?? navigator.paths[navigator.section]?.last?.id else { return }
            withAnimation(.standard) { navigator.showAllSynopses(id, true) }
            Task { @MainActor in
                try? await Task.sleep(for: .milliseconds(1600))
                withAnimation(.standard) { navigator.showAllSynopses(id, false) }
            }
        case .reveal:
            guard let id = navigator.titleOnMac ?? navigator.paths[navigator.section]?.last?.id,
                  let rows = store.titles.episodes[id],
                  let first = EpisodeProtection.startsAt(rows, spoilers: store.spoilerProtection(for: id))
            else { return }
            withAnimation(.standard) { navigator.showSynopsis(id, first) }
        }
    }
}
