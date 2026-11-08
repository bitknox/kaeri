// A title's wide art, by one rule every screen with a hero shares: Continue's preview, a title's
// page on every device, the iPhone's hero and cards, and the player's starting screen.

import KaeriKit
import Foundation

/// What fills a title's wide frame.
///
/// 1. Its AniList banner, when it has one.
/// 2. Else a still the viewer has already seen: the last watched episode's, else episode 1's. A
///    still from further on could give away what happens, so none is ever used. From the episode
///    table once it is here, and before that from the still the engine sends with the title
///    (`Entry.seenStill`), by the same rule from the metadata it holds.
/// 3. On the Mac, else the cover filling the frame: cropped around the part of it that draws the
///    eye, or blurred behind itself where it is too small to crop (the hover card board's No wide
///    art rule, drawn by the screens).
/// 4. Else no wide art at all: the screen drops the frame and sets the cover beside the title.
///
/// The cover never stands in a plate of its own colour for long: that read as empty.
public enum HeroArt: Equatable, Sendable {
    case banner(String)
    case still(String)
    /// No banner and no seen still: the cover fills the frame.
    case cover(String)
    /// No banner and no still known, and the episodes whose stills might stand in are on their
    /// way: the cover's colour as a plate, without the cover, so nothing shows twice. For
    /// `AppStore.heroArtWait` at most, then what there is without a still.
    case waiting
    /// Nothing wide to show.
    case compact

    /// The art for `entry`, with its episode table as far as it is known: `nil` while it loads.
    /// `coverFills` lets the cover fill the frame when there is nothing wider. `waited` says the
    /// plate has shown for long enough: the art is then what it would be with no still.
    public static func resolve(
        _ entry: Entry, episodes: [EpisodeRow]?, coverFills: Bool = false, waited: Bool = false
    ) -> HeroArt {
        if let banner = entry.bannerUrl { return .banner(banner) }
        if let episodes, let still = seenStill(episodes, done: entry.progress?.done ?? 0) { return .still(still) }
        if let still = entry.seenStill { return .still(still) }
        if episodes == nil, !waited { return .waiting }
        if coverFills, let cover = entry.coverUrl { return .cover(cover) }
        return .compact
    }

    /// The still of the latest episode watched that has one, else episode 1's: never one from an
    /// episode not yet seen past the first. Watched is the table's own word, or within what the
    /// tracker counts as done.
    public static func seenStill(_ rows: [EpisodeRow], done: UInt32) -> String? {
        if let still = watchedStill(rows, done: done) { return still }
        let first = rows.first { Double($0.number) == 1 } ?? rows.first
        return first?.thumbnail
    }

    /// The still of the latest episode watched that has one, and nothing else: where the episode
    /// that would stand in unwatched could be the one the card is about, as a Calendar tile's
    /// airing episode is.
    public static func watchedStill(_ rows: [EpisodeRow], done: UInt32) -> String? {
        rows.filter { row in row.completed || Double(row.number).map { $0 <= Double(done) } == true }
            .filter { $0.thumbnail != nil }
            .max { (Double($0.number) ?? 0) < (Double($1.number) ?? 0) }?
            .thumbnail
    }

    /// The URL to draw, when there is one.
    public var url: String? {
        switch self {
        case .banner(let url), .still(let url): url
        case .cover, .waiting, .compact: nil
        }
    }

    public var isCompact: Bool { self == .compact }

    public var isCover: Bool {
        if case .cover = self { true } else { false }
    }
}

extension AppStore {
    /// How long a title's wide art waits on its plate for the episodes that might bring a still:
    /// as long as a screen waits before it shows placeholders. Past it the Mac fills the frame
    /// with the cover, and an iPhone or iPad sets the cover beside the title; a still landing
    /// later takes the frame then.
    public static let heroArtWait: Duration = .milliseconds(600)

    /// `entry`'s wide art, by the episodes known now.
    public func heroArt(_ entry: Entry) -> HeroArt {
        let episodes = titles.episodes[entry.id]
        let waits = entry.bannerUrl == nil && entry.seenStill == nil && episodes == nil
        let waited = waits && heroArtWaited(entry.id)
        #if os(macOS)
        return HeroArt.resolve(entry, episodes: episodes, coverFills: true, waited: waited)
        #else
        return HeroArt.resolve(entry, episodes: episodes, waited: waited)
        #endif
    }
}

