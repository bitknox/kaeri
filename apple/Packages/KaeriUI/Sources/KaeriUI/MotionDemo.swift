// Transitions started by a launch argument, for frame sequences of the motion audit: the Fixtures
// app and the dev harness run one once the store has settled, and the shots record it.

import KaeriKit
import KaeriStore
import SwiftUI
#if os(iOS)
import UIKit
#else
import AppKit
#endif

/// A transition from motion.md the app can be asked to run, with `-KaeriMotion <name>`.
public enum MotionDemo: String, CaseIterable, Sendable {
    /// T1: a cover zooms into its title's page (iPhone and iPad): the first Continue card on Home.
    case t1
    /// T2: the first Continue card's episode played, as its menu's Play does: the player grows
    /// out of the card (iPhone and iPad).
    case t2
    /// T1 from the first poster on Home's first shelf of posters: into the page's cover on the Mac.
    case t1Poster
    /// T1 from Home's spotlight, whose title is often on a shelf too: out of the spotlight.
    case t1Spotlight
    /// T1 from the first cover of the grid on screen: the Library's list, Seasonal, or Search's
    /// results.
    case t1Grid
    /// `t1Grid` double clicked: the same cover clicked again 50 ms later.
    case t1GridTwice
    /// The first title of the screen opened with no element to fly from, as a calendar row does:
    /// the calendar's first broadcast there, else Continue's first title.
    case t1Nowhere
    /// M1 to a section `-KaeriTo` names, as a click in the sidebar does: its first load.
    case section
    /// Home asked for again as on its first visit, with nothing kept: its placeholders, then its
    /// sections arriving.
    case homeFirst
    /// `homeFirst` with no episode table and no wide art wait kept either: the spotlight's and
    /// Continue's wide art as they first come, with `--episodes-delay` holding the tables back.
    case homeArt
    /// M2: Continue's selection moves to the next title, the preview crossfading.
    case m2
    /// M3: the preview's banner grows into the title page.
    case m3
    /// M3 in reverse: Back from a title page opened from the preview.
    case back
    /// Back from the page of Home's spotlight, opened from the spotlight first (`prelude`): its
    /// art flies home.
    case backSpotlight
    /// Back from the spotlight's page as `backSpotlight`, and the spotlight clicked again while
    /// its art is still flying home: the second flight takes off from where the first one is.
    case backReopen
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
    /// Search: Shows switched to Reading, or back, as a click on the scope control does.
    case scope
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
    /// Search: Trending's heading menu switched from the period shown to the next one, Today to
    /// This week (Browsing extras board, loop B).
    case period
    /// Surprise me on the screen's pool, then Another on the page it opened, 2.4 s later
    /// (loop C).
    case surprise
    /// Go to episode on the title page on screen, as Go does: the numbers `-KaeriType` lists,
    /// comma separated, 2 s apart (loop D). By default one in another range, one on the same
    /// screen and one further down.
    case goTo
    /// Digits typed into the Mac's focused episode list, a key every 150 ms: `-KaeriType`'s,
    /// else 237 (loop E).
    case typed
    /// The reader's credit line on page 1 tapped, as its button does: the credit page fades in
    /// over page 1. With `-KaeriReader creditline -KaeriCredits start`.
    case creditShow
    /// Not a credit page on the credit page shown, a turn on 1.6 s later, and Undo 1.6 s after
    /// that: Undo outlasts the turn. With `-KaeriReader creditshown -KaeriCredits start`.
    case creditKeep
    /// A long strip's credit line tapped: Show grows the image in place under its caption. With
    /// a strip scene and `-KaeriReader creditline -KaeriCredits start`.
    case stripShow
    /// The series page's Group row: another group picked, as the picker's row does, the value
    /// crossfading. With `-KaeriGroups`.
    case groupPick

    /// Reduce Motion, as the views read it: the strip grows its credit row at once with it on.
    @MainActor private static var reduceMotion: Bool {
        #if os(iOS)
        UIAccessibility.isReduceMotionEnabled
        #else
        NSWorkspace.shared.accessibilityDisplayShouldReduceMotion
        #endif
    }

    /// What `-KaeriType` gives, for the demos that type.
    private var typed: String? {
        let arguments = ProcessInfo.processInfo.arguments
        guard let at = arguments.firstIndex(of: "-KaeriType"), arguments.indices.contains(at + 1) else { return nil }
        return arguments[at + 1]
    }

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
        case .backSpotlight, .backReopen:
            MotionDemo.t1Spotlight.run(navigator: navigator, store: store)
            try? await Task.sleep(for: .seconds(2))
        case .episodeRange, .protectionLine, .reveal, .goTo, .typed:
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
        case .creditShow, .creditKeep, .stripShow:
            // The reader open and staged: the line up, or the credit page shown.
            for _ in 0..<200 {
                let reader = store.reader
                if self == .creditKeep ? reader.onCredit != nil : reader.phase == .open && !reader.startSkipped.isEmpty
                    && (self == .stripShow || reader.creditLine) { break }
                try? await Task.sleep(for: .milliseconds(50))
            }
        case .groupPick:
            for _ in 0..<200 {
                if store.reading.series.values.contains(where: { ($0.edition?.groups.count ?? 0) > 1 }) { break }
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
        case .t2:
            guard let item = store.discovery.view?.sections.first(where: { $0.kind == .continue })?.items.first,
                  let episode = item.episode
            else { return }
            navigator.playingFrom = .shelf(item.entry.id, .continue, art: .poster)
            store.send(.play(id: item.entry.id, episode: String(episode)))
        case .t1Poster:
            let shelves = store.discovery.view?.sections.filter { $0.kind != .continue && $0.kind != .thisWeek } ?? []
            guard let shelf = shelves.first(where: { !$0.items.isEmpty }), let item = shelf.items.first else { return }
            navigator.openTitle(item.entry.id, from: .shelf(item.entry.id, shelf.kind, art: .poster))
        case .t1Spotlight:
            guard let spot = store.discovery.view?.spotlight else { return }
            let entry = store.titles[spot.entry.id] ?? spot.entry
            navigator.openTitle(entry.id, from: TitleSource(entry.id, .spotlight, art: store.heroArt(entry).isCompact ? .poster : .banner))
        case .t1Grid:
            let ids = switch navigator.section {
            case .seasonal: store.seasonal.state.ids
            case .search: store.search.list.state.ids
            default: store.library.segment(navigator.librarySegment).state.ids
            }
            guard let id = ids.first else { return }
            navigator.openTitle(id, from: TitleSource(id, .grid, art: .poster))
        case .t1GridTwice:
            MotionDemo.t1Grid.run(navigator: navigator, store: store)
            Task { @MainActor in
                try? await Task.sleep(for: .milliseconds(50))
                MotionDemo.t1Grid.run(navigator: navigator, store: store)
            }
        case .t1Nowhere:
            let first = navigator.section == .calendar
                ? store.broadcasts(mine: store.calendarMine).first?.entry.id
                : queue.first ?? store.seasonal.state.ids.first
            guard let id = first else { return }
            navigator.openTitle(id)
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
        case .homeArt:
            store.discovery.forgetForDemo()
            store.forgetArtForDemo()
            store.loadHome(force: true)
        case .m2:
            if queue.count > 1 { navigator.continueSelection = queue[1] }
        case .m3:
            if let id = navigator.focusedTitle ?? queue.first { navigator.openTitle(id) }
        case .back, .backSpotlight:
            navigator.back()
        case .backReopen:
            navigator.back()
            Task { @MainActor in
                try? await Task.sleep(for: .milliseconds(200))
                MotionDemo.t1Spotlight.run(navigator: navigator, store: store)
            }
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
        case .scope:
            ReadSearchScope.choose(!navigator.searchReading, store: store, navigator: navigator)
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
        case .period:
            store.loadTrending(store.search.trendingPeriod == .today ? .week : .today)
        case .surprise:
            guard let pool = surprisePool(store: store, navigator: navigator) else { return }
            navigator.anotherAsked = false
            store.surprise(pool)
            Task { @MainActor in
                try? await Task.sleep(for: .milliseconds(2400))
                guard let visit = navigator.pickVisit else { return }
                navigator.anotherAsked = true
                store.surprise(visit.pool)
            }
        case .goTo:
            guard let id = navigator.titleOnMac ?? navigator.paths[navigator.section]?.last?.id,
                  let loaded = store.titles.episodes[id], let entry = store.titles[id]
            else { return }
            let rows = entry.episodeTable(loaded)
            let numbers = (typed ?? "137,140,190").split(separator: ",").map(String.init)
            Task { @MainActor in
                for (at, number) in numbers.enumerated() {
                    if at > 0 { try? await Task.sleep(for: .seconds(2)) }
                    if case .row(let index, let range) = goToResult(rows: rows, typed: number) {
                        navigator.land(on: id, rows: rows, index: Int(index), range: range.map(Int.init))
                    }
                }
            }
        case .typed:
            guard let id = navigator.titleOnMac ?? navigator.paths[navigator.section]?.last?.id,
                  let loaded = store.titles.episodes[id], let entry = store.titles[id]
            else { return }
            let rows = entry.episodeTable(loaded)
            Task { @MainActor in
                for digit in typed ?? "237" {
                    navigator.typeEpisode(String(digit), title: id, rows: rows)
                    try? await Task.sleep(for: .milliseconds(150))
                }
            }
        case .creditShow:
            // The credit line's own action (ReaderChrome).
            withAnimation(.fadeIn) { store.reader.showStartCredits() }
        case .creditKeep:
            // The notice's button, then a turn and Undo, as the person would.
            let reader = store.reader
            reader.act(.keepCredit)
            Task { @MainActor in
                try? await Task.sleep(for: .milliseconds(1600))
                reader.requestTurn(forward: true)
                try? await Task.sleep(for: .milliseconds(1600))
                reader.act(.undoKeep)
            }
        case .stripShow:
            // The strip line's own action (StripCredits).
            let reader = store.reader
            guard let chapter = reader.view?.chapter.id else { return }
            withAnimation(Self.reduceMotion ? nil : .standard) { reader.reveal(reader.startSkipped, chapter: chapter) }
        case .groupPick:
            // The picker row's pick (GroupPicker), on the series with groups.
            guard let view = store.reading.series.values.first(where: { ($0.edition?.groups.count ?? 0) > 1 }),
                  let other = view.edition?.groups.first(where: { !$0.inUse })
            else { return }
            store.reading.chooseGroup(view.work, group: other.label)
        }
    }
}
